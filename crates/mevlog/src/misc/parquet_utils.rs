use arrow::{array::Array, record_batch::RecordBatch};

/// Extract a string value from a parquet column at the specified row index.
/// Handles multiple Arrow data types and converts them to string representation.
pub fn get_parquet_string_value(batch: &RecordBatch, col_idx: usize, row_idx: usize) -> String {
    let column = batch.column(col_idx);
    if column.is_null(row_idx) {
        return String::new();
    }

    match column.data_type() {
        arrow::datatypes::DataType::Utf8 => {
            let string_array = column
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .unwrap();
            string_array.value(row_idx).to_string()
        }
        arrow::datatypes::DataType::LargeUtf8 => {
            let string_array = column
                .as_any()
                .downcast_ref::<arrow::array::LargeStringArray>()
                .unwrap();
            string_array.value(row_idx).to_string()
        }
        arrow::datatypes::DataType::LargeBinary => {
            let binary_array = column
                .as_any()
                .downcast_ref::<arrow::array::LargeBinaryArray>()
                .unwrap();
            let bytes = binary_array.value(row_idx);
            format!("0x{}", hex::encode(bytes))
        }
        arrow::datatypes::DataType::Int64 => {
            let int_array = column
                .as_any()
                .downcast_ref::<arrow::array::Int64Array>()
                .unwrap();
            int_array.value(row_idx).to_string()
        }
        arrow::datatypes::DataType::UInt64 => {
            let uint_array = column
                .as_any()
                .downcast_ref::<arrow::array::UInt64Array>()
                .unwrap();
            uint_array.value(row_idx).to_string()
        }
        arrow::datatypes::DataType::UInt32 => {
            let uint_array = column
                .as_any()
                .downcast_ref::<arrow::array::UInt32Array>()
                .unwrap();
            uint_array.value(row_idx).to_string()
        }
        arrow::datatypes::DataType::Float64 => {
            let float_array = column
                .as_any()
                .downcast_ref::<arrow::array::Float64Array>()
                .unwrap();
            float_array.value(row_idx).to_string()
        }
        arrow::datatypes::DataType::Boolean => {
            let bool_array = column
                .as_any()
                .downcast_ref::<arrow::array::BooleanArray>()
                .unwrap();
            bool_array.value(row_idx).to_string()
        }
        _ => {
            // For other types, try to get string representation
            format!("{:?}", column.slice(row_idx, 1))
        }
    }
}
