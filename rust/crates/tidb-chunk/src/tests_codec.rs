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

//! Ports of `pkg/util/chunk/codec_test.go`.

use std::sync::Arc;

use bytes::Bytes;
use tidb_datatype::{BinaryJSON, BinaryJSONValue, Datum, FieldType, FieldTypeCode, MyDecimal};

use crate::chunk::Chunk;
use crate::codec::Codec;
use crate::mutrow::MutRow;

fn codec_field_types() -> Vec<FieldType> {
    vec![
        FieldType::new(FieldTypeCode::LongLong),
        FieldType::new(FieldTypeCode::LongLong),
        FieldType::new(FieldTypeCode::Varchar),
        FieldType::new(FieldTypeCode::Varchar),
        FieldType::new(FieldTypeCode::NewDecimal),
        FieldType::new(FieldTypeCode::Json),
    ]
}

/// Go `TestCodec` (codec_test.go): encode a mixed chunk and decode it back.
#[test]
fn codec_encode_decode_round_trip() {
    let num_rows = 10;
    let col_types = codec_field_types();

    let mut old_chk = Chunk::new_chunk_with_capacity(&col_types, num_rows);
    for i in 0..num_rows {
        let text = format!("{i}.12345");
        old_chk.append_null(0);
        old_chk.append_int64(1, i as i64);
        old_chk.append_string(2, text.as_bytes());
        old_chk.append_string(3, text.as_bytes());
        let decimal = MyDecimal::from_string(text.as_bytes()).0;
        old_chk.append_my_decimal(4, &decimal);
        let json = create_string_json(&text);
        old_chk.append_json(5, &json);
    }

    let codec = Codec::new(col_types.clone());
    let buffer = codec.encode(&old_chk);

    let mut new_chk = Chunk::new_chunk_with_capacity(&col_types, num_rows);
    let remained = codec.decode_to_chunk(&buffer, &mut new_chk);

    assert!(remained.is_empty());
    assert_eq!(new_chk.num_cols(), col_types.len());
    assert_eq!(new_chk.num_rows(), num_rows);
    for i in 0..num_rows {
        let row = new_chk.get_row(i);
        let text = format!("{i}.12345");
        assert!(row.is_null(0));
        assert!(!row.is_null(1));
        assert!(!row.is_null(2));
        assert!(!row.is_null(3));
        assert!(!row.is_null(4));
        assert!(!row.is_null(5));

        assert_eq!(row.get_int64(1), i as i64);
        assert_eq!(row.get_string(2), text.as_str());
        assert_eq!(row.get_string(3), text.as_str());
        let expected_decimal = MyDecimal::from_string(text.as_bytes()).0;
        assert_eq!(row.get_my_decimal(4), expected_decimal);
        assert_eq!(row.get_json(5), create_string_json(&text));
    }
}

#[test]
fn frozen_data_shares_the_visible_range_and_keeps_its_owner_alive() {
    let fields = [FieldType::new(FieldTypeCode::Varchar)];
    let codec = Codec::new(fields.to_vec());
    let mut chunk = Chunk::new_chunk_with_capacity(&fields, 3);
    chunk.append_bytes(0, b"left");
    chunk.append_bytes(0, b"keep");
    chunk.append_bytes(0, b"right");

    let owner: Arc<[u8]> = codec.encode(&chunk).into();
    let lifetime = Arc::downgrade(&owner);
    let input = Bytes::from_owner(owner);
    let cell_ptr = input[input.len() - b"keepright".len()..].as_ptr();
    assert!(codec
        .try_decode_bytes_to_chunk(&input, &mut chunk)
        .unwrap()
        .is_empty());
    drop(input);

    let mut cell = MutRow::mut_row_from_types(&fields);
    cell.shallow_copy_partial_row(0, &mut chunk, 1);
    let cell_chunk = cell.to_row().chunk().unwrap();
    let retained = cell_chunk.column(0).frozen_data().unwrap();
    assert_eq!(retained.as_ptr(), cell_ptr);
    assert_eq!(retained.as_ref(), b"keep");

    // A same-size write to the shallow cell must detach, not edit the image.
    cell.set_value(0, &Datum::Bytes(b"edit".to_vec()));
    assert_eq!(cell.to_row().get_bytes(0).as_ref(), b"edit");
    let detached = cell.to_row().chunk().unwrap();
    assert!(detached.column(0).frozen_data().is_none());
    assert_eq!(chunk.get_row(1).get_bytes(0).as_ref(), b"keep");
    drop(cell);

    chunk.reset();
    assert!(chunk.column(0).frozen_data().unwrap().is_empty());
    chunk.append_bytes(0, b"reused");
    assert!(chunk.column(0).frozen_data().is_none());
    drop(chunk);

    assert!(lifetime.upgrade().is_some());
    assert_eq!(retained.as_ref(), b"keep");
    drop(retained);
    assert!(lifetime.upgrade().is_none());
}

#[test]
fn frozen_data_is_unavailable_for_owned_and_mutably_shared_columns() {
    let fields = [FieldType::new(FieldTypeCode::Varchar)];
    let mut chunk = Chunk::new_chunk_with_capacity(&fields, 1);
    chunk.append_bytes(0, b"old");
    assert!(chunk.column(0).frozen_data().is_none());

    // Decoding a borrowed slice copies the column data despite marking it avoid_reusing.
    let codec = Codec::new(fields.to_vec());
    let image = codec.encode(&chunk);
    assert!(codec.decode_to_chunk(&image, &mut chunk).is_empty());
    assert!(chunk.column(0).frozen_data().is_none());

    let mut cell = MutRow::mut_row_from_types(&fields);
    cell.shallow_copy_partial_row(0, &mut chunk, 0);
    assert!(chunk.column(0).frozen_data().is_none());
    cell.set_value(0, &Datum::Bytes(b"new".to_vec()));
    assert_eq!(chunk.get_row(0).get_bytes(0).as_ref(), b"new");
}

/// Go `types.CreateBinaryJSON(str)`: a binary JSON string value.
fn create_string_json(text: &str) -> BinaryJSON {
    BinaryJSON::from_typed_value(&BinaryJSONValue::String(text.to_owned()))
        .expect("string json value")
}
