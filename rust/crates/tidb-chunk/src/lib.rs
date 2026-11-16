// Copyright 2026 PingCAP, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! `pkg/util/chunk`: the columnar row container that every executor produces
//! and every expression `Eval*` consumes.
//!
//! [`column`] provides packed fixed- and variable-width storage, guarded byte
//! views, resize/reserve/reset operations, and the typed values consumed by
//! expression evaluation. [`chunk`] owns a batch of columns and [`row`] is its
//! read cursor. Go strings remain byte-authoritative and are represented by
//! `tidb_datatype::GoString` at the value boundary.
//!
//! [`mutrow`] adds Go's `MutRow`, the mutable one-row chunk that partition
//! pruning and ranger detachment evaluate expressions against.
//!
//! [`compare`] adds Go `compare.go` whole: `GetCompareFunc`, `Compare` against
//! a `Datum`, and `Chunk::lower_bound`/`upper_bound`. [`list`] and [`iterator`]
//! add Go `list.go`/`iterator.go` whole -- the unbounded in-memory chunk
//! sequence and the five standalone iterators. [`row_container`] adds the
//! shared in-memory/disk storage root and quota spill action, while
//! [`row_container_reader`] provides its chunk-at-a-time forward reader.
//!
//! [`chunk_in_disk`] adds the spill-to-disk container `DataInDiskByChunks`
//! (Go `chunk_in_disk.go`) over the checksum-framed temporary file in
//! [`chunk_util`]; [`row_in_disk`] carries the read-while-writing reader it
//! needs.

pub mod alloc;
pub mod chunk;
pub mod chunk_in_disk;
pub mod chunk_util;
pub mod codec;
pub mod column;
mod column_slot;
mod go_append;
mod column_view;
pub mod compare;
pub mod iterator;
pub mod list;
pub mod mutrow;
pub mod pool;
pub mod raw_column;
pub mod row;
pub mod row_container;
pub mod row_container_reader;
pub mod row_in_disk;
mod shared_bytes;
pub mod sorted_row_container;

#[cfg(test)]
mod tests_chunk_util;
#[cfg(test)]
mod tests_codec;

pub use column_slot::{ColumnHandle, ColumnRead, ColumnRef, ColumnWrite};
pub use column_view::{CellBytes, ColumnBytes};

#[cfg(test)]
mod test_temp_storage_process_tests;

/// Test-only storage authorities. Each process configures one private global
/// temporary path before any fixture can reach the disk initializer; ordinary
/// tests within the process still run concurrently.
#[cfg(test)]
pub(crate) mod test_temp_storage {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, OnceLock};

    use tidb_spill_storage::{SpillEncryptionMethod, SpillStorage, SpillStorageSpec};

    fn configure_process_temp_storage() {
        // Retain the owned parent for the process lifetime, but do not create
        // its child: the first real Chunk spill must initialize it and acquire
        // the production directory lock cold. OnceLock guards configuration
        // publication only, not test execution or I/O.
        static ROOT: OnceLock<tempfile::TempDir> = OnceLock::new();
        ROOT.get_or_init(|| {
            let root = tempfile::Builder::new()
                .prefix(&format!("tidb-chunk-process-{}-", std::process::id()))
                .tempdir()
                .expect("create process temporary-storage parent");
            let path = root.path().join("global");
            tidb_config::config_tree::config::update_global(|config| {
                config.temp_storage_path = path.to_string_lossy().into_owned();
            });
            root
        });
    }

    /// One plaintext authority shared by ordinary unit tests. Production never
    /// has an implicit process fallback; this exists only to keep test setup
    /// focused on the chunk behavior under test.
    pub(crate) fn storage() -> Arc<SpillStorage> {
        static STORAGE: OnceLock<Arc<SpillStorage>> = OnceLock::new();
        Arc::clone(
            STORAGE.get_or_init(|| isolated_storage("shared", SpillEncryptionMethod::Plaintext)),
        )
    }

    /// A separately configured authority for path/encryption-sensitive tests.
    pub(crate) fn isolated_storage(
        name: &str,
        encryption: SpillEncryptionMethod,
    ) -> Arc<SpillStorage> {
        isolated_storage_with_quota(name, encryption, -1)
    }

    /// A separately configured authority with an exact quota.
    pub(crate) fn isolated_storage_with_quota(
        name: &str,
        encryption: SpillEncryptionMethod,
        quota_bytes: i64,
    ) -> Arc<SpillStorage> {
        configure_process_temp_storage();
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let ordinal = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "tidb_rust_spill_test_{}_{}_{}_{}",
            std::process::id(),
            name,
            ordinal,
            encryption.as_config_value()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        // Production guarantees `tidb_spill_dir` exists (the server owns it);
        // a fresh test process starts with nothing, so create the scratch
        // directory here or every writer's first `create` fails NotFound.
        std::fs::create_dir_all(&dir).expect("create test spill directory");
        Arc::new(
            SpillStorage::open(SpillStorageSpec {
                path: dir,
                quota_bytes,
                encryption,
            })
            .expect("test spill storage"),
        )
    }
}
