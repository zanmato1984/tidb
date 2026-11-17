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

//! Native process coverage for the test fixture, not a substitute lock model.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use tidb_config::config_tree::config;
use tidb_datatype::{FieldType, FieldTypeCode};
use tidb_spill_storage::SpillEncryptionMethod;

use crate::chunk::Chunk;
use crate::chunk_in_disk::DataInDiskByChunks;

const TEST: &str =
    "test_temp_storage_process_tests::process_isolated_fixture_spills_and_preserves_lock_rejection";
const ROLE: &str = "TIDB_CHUNK_FIXTURE_PROCESS_ROLE";
const MARKER: &str = "CHUNK_FIXTURE_IPC\t";

struct NativeChild {
    process: Child,
    events: Receiver<String>,
}

impl NativeChild {
    fn start(role: &str, suite_tmp: &Path, shared_path: Option<&Path>) -> Self {
        let mut command = Command::new(std::env::current_exe().expect("native test executable"));
        command
            .args(["--exact", TEST, "--nocapture"])
            .env(ROLE, role)
            .env("TMPDIR", suite_tmp)
            .env("TMP", suite_tmp)
            .env("TEMP", suite_tmp)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if let Some(path) = shared_path {
            command.env("TIDB_CHUNK_FIXTURE_SHARED_PATH", path);
        }
        let mut process = command.spawn().expect("start native fixture child");
        let output = process.stdout.take().expect("child stdout");
        let (send, events) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let line = line.expect("child stdout line");
                if let Some((_, event)) = line.split_once(MARKER) {
                    if send.send(event.to_owned()).is_err() {
                        break;
                    }
                }
            }
        });
        Self { process, events }
    }

    fn event(&self, expected: &str) -> String {
        let event = self
            .events
            .recv_timeout(Duration::from_secs(30))
            .expect("native child must answer the IPC barrier within 30 seconds");
        eprintln!("native child {}: {event}", self.process.id());
        let (kind, value) = event.split_once('\t').expect("IPC event and value");
        assert_eq!(kind, expected);
        value.to_owned()
    }

    fn command(&mut self, command: &str) {
        let input = self.process.stdin.as_mut().expect("child stdin");
        writeln!(input, "{command}").expect("send child barrier");
        input.flush().expect("flush child barrier");
    }

    fn finish(&mut self) {
        self.command("release");
        assert!(self.process.wait().expect("wait native child").success());
    }
}

impl Drop for NativeChild {
    fn drop(&mut self) {
        // Also releases children after any parent assertion/IPC failure.
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

fn event(kind: &str, value: &str) {
    println!("{MARKER}{kind}\t{value}");
    std::io::stdout().flush().expect("flush IPC event");
}

fn barrier(expected: &str) {
    let mut command = String::new();
    std::io::stdin()
        .read_line(&mut command)
        .expect("read parent barrier");
    assert_eq!(command.trim(), expected);
}

fn spill_known_values(storage: std::sync::Arc<tidb_spill_storage::SpillStorage>) {
    let fields = vec![FieldType::new(FieldTypeCode::LongLong)];
    let mut chunk = Chunk::new_chunk_with_capacity(&fields, 1024);
    for row in 0..1024 {
        chunk.append_int64(0, row * 17 - 9);
    }
    let mut disk = DataInDiskByChunks::new(fields, "process-fixture", storage);
    disk.add(&chunk).expect("cold real Chunk spill");
    let path = disk.file_path().cloned().expect("actual spill file");
    assert!(std::fs::metadata(&path).expect("spill metadata").len() > 0);
    let restored = disk.get_chunk(0).expect("read actual spilled Chunk");
    assert_eq!(restored.num_rows(), 1024);
    for row in 0..1024 {
        assert_eq!(restored.get_row(row).get_int64(0), row as i64 * 17 - 9);
    }
    disk.close();
    assert!(!path.exists(), "close removes the actual spill file");
}

fn fixture_child(role: &str) {
    if role == "same-path" {
        // This child runs only this exact test, never ordinary same-process
        // tests. Deliberately request the holder's path through the real API.
        let path = std::env::var("TIDB_CHUNK_FIXTURE_SHARED_PATH").expect("held global path");
        config::update_global(|config| config.temp_storage_path = path);
        let error = tidb_disk::initialize_temp_dir().expect_err("real held path must reject");
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        event("rejected", "WouldBlock");
        barrier("release");
        return;
    }
    let storage = match role {
        "holder" => crate::test_temp_storage::storage(),
        "contender" => crate::test_temp_storage::isolated_storage_with_quota(
            "direct-quota-route",
            SpillEncryptionMethod::Plaintext,
            -1,
        ),
        _ => panic!("unknown native child role"),
    };
    let global = PathBuf::from(&config::get_global_config().temp_storage_path);
    assert!(!global.exists(), "fixture must leave the global path cold");
    event("cold", global.to_str().expect("UTF-8 test path"));
    barrier("spill");
    spill_known_values(storage);
    assert!(
        global.join("_dir.lock").is_file(),
        "real global kernel lock"
    );
    event("held", global.to_str().expect("UTF-8 test path"));
    barrier("release");
    tidb_disk::clean_up();
}

#[test]
fn process_isolated_fixture_spills_and_preserves_lock_rejection() {
    if let Ok(role) = std::env::var(ROLE) {
        fixture_child(&role);
        return;
    }
    let suite = tempfile::Builder::new()
        .prefix("tidb-chunk-common-suite-")
        .tempdir()
        .expect("common suite TMPDIR");
    let mut holder = NativeChild::start("holder", suite.path(), None);
    let holder_path = PathBuf::from(holder.event("cold"));
    let mut contender = NativeChild::start("contender", suite.path(), None);
    let contender_path = PathBuf::from(contender.event("cold"));

    // Both processes reached a cold fixture before either initializes disk.
    // Keep the holder alive and its real lock held during the other workload.
    holder.command("spill");
    assert_eq!(PathBuf::from(holder.event("held")), holder_path);
    assert!(holder.process.try_wait().expect("holder alive").is_none());
    contender.command("spill");
    assert_eq!(PathBuf::from(contender.event("held")), contender_path);
    assert!(holder
        .process
        .try_wait()
        .expect("holder still alive")
        .is_none());

    let mut same_path = NativeChild::start("same-path", suite.path(), Some(&holder_path));
    assert_eq!(same_path.event("rejected"), "WouldBlock");
    same_path.finish();

    // In the old fixture both real workloads use the same global path. Check
    // after exercising I/O and the adverse control so red is not a mock proof.
    assert_ne!(
        holder_path, contender_path,
        "actual fixture processes must not share their global temporary path"
    );
    assert!(holder_path.starts_with(suite.path()));
    assert!(contender_path.starts_with(suite.path()));
    contender.finish();
    holder.finish();
}
