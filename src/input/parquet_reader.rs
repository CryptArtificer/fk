//! Parquet file reader: reads rows as records with pre-split fields.
//!
//! This module is only compiled when the `parquet` feature is enabled.

use std::io;

/// Read all records from a Parquet file.
/// Returns (column_names, rows) where each row is a Vec of string field values.
pub fn read_parquet_file(path: &str) -> io::Result<(Vec<String>, Vec<Vec<String>>)> {
    use parquet::arrow::arrow_reader::{ArrowReaderOptions, ParquetRecordBatchReaderBuilder};
    use std::fs::File;

    let file = File::open(path)
        .map_err(|e| io::Error::new(e.kind(), format!("fk: {}: {}", path, e)))?;

    let options = ArrowReaderOptions::new().with_skip_arrow_metadata(true);
    let builder = ParquetRecordBatchReaderBuilder::try_new_with_options(file, options)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("fk: parquet: {}", e)))?;

    let schema = builder.schema().clone();
    let columns: Vec<String> = schema.fields().iter().map(|f| f.name().clone()).collect();

    let reader = builder.build()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("fk: parquet: {}", e)))?;

    let mut rows = Vec::new();

    for batch_result in reader {
        let batch = batch_result
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("fk: parquet: {}", e)))?;

        let num_rows = batch.num_rows();
        let num_cols = batch.num_columns();

        for row_idx in 0..num_rows {
            let mut fields = Vec::with_capacity(num_cols);
            for col_idx in 0..num_cols {
                let col = batch.column(col_idx);
                let val = array_value_to_string(col, row_idx);
                fields.push(val);
            }
            rows.push(fields);
        }
    }

    Ok((columns, rows))
}

fn array_value_to_string(array: &dyn arrow::array::Array, idx: usize) -> String {
    use arrow::array::*;
    use arrow::datatypes::DataType;

    if array.is_null(idx) {
        return String::new();
    }

    match array.data_type() {
        DataType::Boolean => {
            let a = array.as_any().downcast_ref::<BooleanArray>().unwrap();
            if a.value(idx) { "1".to_string() } else { "0".to_string() }
        }
        DataType::Int8 => {
            let a = array.as_any().downcast_ref::<Int8Array>().unwrap();
            a.value(idx).to_string()
        }
        DataType::Int16 => {
            let a = array.as_any().downcast_ref::<Int16Array>().unwrap();
            a.value(idx).to_string()
        }
        DataType::Int32 => {
            let a = array.as_any().downcast_ref::<Int32Array>().unwrap();
            a.value(idx).to_string()
        }
        DataType::Int64 => {
            let a = array.as_any().downcast_ref::<Int64Array>().unwrap();
            a.value(idx).to_string()
        }
        DataType::UInt8 => {
            let a = array.as_any().downcast_ref::<UInt8Array>().unwrap();
            a.value(idx).to_string()
        }
        DataType::UInt16 => {
            let a = array.as_any().downcast_ref::<UInt16Array>().unwrap();
            a.value(idx).to_string()
        }
        DataType::UInt32 => {
            let a = array.as_any().downcast_ref::<UInt32Array>().unwrap();
            a.value(idx).to_string()
        }
        DataType::UInt64 => {
            let a = array.as_any().downcast_ref::<UInt64Array>().unwrap();
            a.value(idx).to_string()
        }
        DataType::Float32 => {
            let a = array.as_any().downcast_ref::<Float32Array>().unwrap();
            let v = a.value(idx);
            if v == v.trunc() && v.abs() < 1e15 {
                format!("{}", v as i64)
            } else {
                format!("{}", v)
            }
        }
        DataType::Float64 => {
            let a = array.as_any().downcast_ref::<Float64Array>().unwrap();
            let v = a.value(idx);
            if v == v.trunc() && v.abs() < 1e15 {
                format!("{}", v as i64)
            } else {
                format!("{}", v)
            }
        }
        DataType::Utf8 => {
            let a = array.as_any().downcast_ref::<StringArray>().unwrap();
            a.value(idx).to_string()
        }
        DataType::LargeUtf8 => {
            let a = array.as_any().downcast_ref::<LargeStringArray>().unwrap();
            a.value(idx).to_string()
        }
        DataType::Date32 => {
            let a = array.as_any().downcast_ref::<Date32Array>().unwrap();
            let days = a.value(idx) as i64;
            let secs = days * 86400;
            format_epoch_date(secs)
        }
        DataType::Date64 => {
            let a = array.as_any().downcast_ref::<Date64Array>().unwrap();
            let ms = a.value(idx);
            let secs = ms / 1000;
            format_epoch_datetime(secs)
        }
        DataType::Timestamp(_, _) => {
            let a = array.as_any().downcast_ref::<TimestampMicrosecondArray>();
            if let Some(a) = a {
                let us = a.value(idx);
                format_epoch_datetime(us / 1_000_000)
            } else {
                let a = array.as_any().downcast_ref::<TimestampMillisecondArray>();
                if let Some(a) = a {
                    format_epoch_datetime(a.value(idx) / 1000)
                } else {
                    let a = array.as_any().downcast_ref::<TimestampSecondArray>();
                    if let Some(a) = a {
                        format_epoch_datetime(a.value(idx))
                    } else {
                        let a = array.as_any().downcast_ref::<TimestampNanosecondArray>();
                        if let Some(a) = a {
                            format_epoch_datetime(a.value(idx) / 1_000_000_000)
                        } else {
                            String::new()
                        }
                    }
                }
            }
        }
        DataType::Dictionary(_, _) => {
            if let Some(a) = array.as_any().downcast_ref::<arrow::array::DictionaryArray<arrow::datatypes::UInt32Type>>() {
                let values = a.values();
                let key = a.keys().value(idx) as usize;
                array_value_to_string(values.as_ref(), key)
            } else if let Some(a) = array.as_any().downcast_ref::<arrow::array::DictionaryArray<arrow::datatypes::Int32Type>>() {
                let values = a.values();
                let key = a.keys().value(idx) as usize;
                array_value_to_string(values.as_ref(), key)
            } else if let Some(a) = array.as_any().downcast_ref::<arrow::array::DictionaryArray<arrow::datatypes::UInt16Type>>() {
                let values = a.values();
                let key = a.keys().value(idx) as usize;
                array_value_to_string(values.as_ref(), key)
            } else if let Some(a) = array.as_any().downcast_ref::<arrow::array::DictionaryArray<arrow::datatypes::Int16Type>>() {
                let values = a.values();
                let key = a.keys().value(idx) as usize;
                array_value_to_string(values.as_ref(), key)
            } else if let Some(a) = array.as_any().downcast_ref::<arrow::array::DictionaryArray<arrow::datatypes::UInt8Type>>() {
                let values = a.values();
                let key = a.keys().value(idx) as usize;
                array_value_to_string(values.as_ref(), key)
            } else if let Some(a) = array.as_any().downcast_ref::<arrow::array::DictionaryArray<arrow::datatypes::Int8Type>>() {
                let values = a.values();
                let key = a.keys().value(idx) as usize;
                array_value_to_string(values.as_ref(), key)
            } else {
                use arrow::util::display::ArrayFormatter;
                ArrayFormatter::try_new(array, &Default::default())
                    .map(|f| f.value(idx).to_string())
                    .unwrap_or_default()
            }
        }
        _ => {
            use arrow::util::display::ArrayFormatter;
            let fmt = ArrayFormatter::try_new(array, &Default::default());
            match fmt {
                Ok(f) => f.value(idx).to_string(),
                Err(_) => String::new(),
            }
        }
    }
}

fn format_epoch_date(secs: i64) -> String {
    let days = secs / 86400;
    let mut y = 1970i64;
    let mut remaining = days;
    loop {
        let yd = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if remaining < yd { break; }
        remaining -= yd;
        y += 1;
    }
    let months = [31, if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 29 } else { 28 },
                  31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0;
    for &md in &months {
        if remaining < md { break; }
        remaining -= md;
        m += 1;
    }
    format!("{:04}-{:02}-{:02}", y, m + 1, remaining + 1)
}

fn format_epoch_datetime(secs: i64) -> String {
    let date = format_epoch_date(secs);
    let day_secs = secs.rem_euclid(86400);
    let h = day_secs / 3600;
    let m = (day_secs % 3600) / 60;
    let s = day_secs % 60;
    format!("{}T{:02}:{:02}:{:02}Z", date, h, m, s)
}
