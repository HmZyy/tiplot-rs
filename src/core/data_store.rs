use arrow::array::{
    Array, BooleanArray, Float32Array, Float64Array, Int16Array, Int32Array, Int64Array, Int8Array,
    StringArray, UInt16Array, UInt32Array, UInt64Array, UInt8Array,
};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use std::collections::HashMap;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct DataStore {
    pub topics: HashMap<String, HashMap<String, Vec<f32>>>,

    pub start_time: f32,
}

impl DataStore {
    pub fn new() -> Self {
        Self {
            topics: HashMap::new(),
            start_time: 0.0,
        }
    }

    pub fn ingest(&mut self, topic: String, batch: RecordBatch) {
        let schema = batch.schema();

        let time_offset = self.start_time;

        let entry = self.topics.entry(topic).or_default();
        for (i, field) in schema.fields().iter().enumerate() {
            let col_name = field.name();
            let column = batch.column(i);

            Self::convert_and_append_static(column, col_name, time_offset, entry);
        }
    }

    fn convert_and_append_static(
        column: &dyn Array,
        col_name: &str,
        time_offset: f32,
        entry: &mut HashMap<String, Vec<f32>>,
    ) {
        let target = entry.entry(col_name.to_string()).or_default();

        if let Some(arr) = column.as_any().downcast_ref::<Float32Array>() {
            target.extend(arr.values());
        } else if let Some(arr) = column.as_any().downcast_ref::<Float64Array>() {
            target.extend(arr.values().iter().map(|&v| v as f32));
        } else if let Some(arr) = column.as_any().downcast_ref::<Int8Array>() {
            target.extend(arr.values().iter().map(|&v| v as f32));
        } else if let Some(arr) = column.as_any().downcast_ref::<Int16Array>() {
            target.extend(arr.values().iter().map(|&v| v as f32));
        } else if let Some(arr) = column.as_any().downcast_ref::<Int32Array>() {
            target.extend(arr.values().iter().map(|&v| v as f32));
        } else if let Some(arr) = column.as_any().downcast_ref::<Int64Array>() {
            if col_name == "timestamp" {
                let time_offset_f64 = time_offset as f64;
                target.extend(arr.values().iter().map(|&v| {
                    let seconds = v as f64 / 1_000_000.0;
                    let normalized = seconds - time_offset_f64;
                    normalized as f32
                }));
            } else {
                target.extend(arr.values().iter().map(|&v| v as f32));
            }
        } else if let Some(arr) = column.as_any().downcast_ref::<UInt8Array>() {
            target.extend(arr.values().iter().map(|&v| v as f32));
        } else if let Some(arr) = column.as_any().downcast_ref::<UInt16Array>() {
            target.extend(arr.values().iter().map(|&v| v as f32));
        } else if let Some(arr) = column.as_any().downcast_ref::<UInt32Array>() {
            target.extend(arr.values().iter().map(|&v| v as f32));
        } else if let Some(arr) = column.as_any().downcast_ref::<UInt64Array>() {
            if col_name == "timestamp" {
                target.extend(arr.values().iter().map(|&v| {
                    let seconds = (v as f64 / 1_000_000.0) as f32;
                    seconds - time_offset
                }));
            } else {
                target.extend(arr.values().iter().map(|&v| v as f32));
            }
        } else if let Some(arr) = column.as_any().downcast_ref::<BooleanArray>() {
            target.extend(arr.values().iter().map(|v| if v { 1.0 } else { 0.0 }));
        } else if let Some(arr) = column.as_any().downcast_ref::<StringArray>() {
            target.extend(arr.iter().map(|v| {
                v.map(|s| {
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    s.hash(&mut hasher);
                    (hasher.finish() as f32) % 1000.0
                })
                .unwrap_or(f32::NAN)
            }));
        } else {
            eprintln!(
                "Warning: Unhandled Arrow type for column '{}': {:?}",
                col_name,
                column.data_type()
            );
        }
    }

    pub fn save_to_arrow<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        use arrow::ipc::writer::StreamWriter;

        if self.topics.is_empty() {
            return Err(anyhow::anyhow!("No data to save"));
        }

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        let valid_topics: Vec<_> = self
            .topics
            .iter()
            .filter(|(topic_name, columns)| {
                if columns.is_empty() {
                    println!("  Skipping empty topic: {}", topic_name);
                    return false;
                }

                let has_data = columns.values().any(|v| !v.is_empty());
                if !has_data {
                    println!("  Skipping topic with no data: {}", topic_name);
                    return false;
                }

                true
            })
            .collect();

        writer.write_all(&(valid_topics.len() as u32).to_le_bytes())?;
        writer.write_all(&self.start_time.to_le_bytes())?;

        for (topic_name, columns) in valid_topics {
            let mut column_names: Vec<_> = columns.keys().cloned().collect();
            column_names.sort();
            let mut fields = Vec::new();
            let mut arrays: Vec<Arc<dyn Array>> = Vec::new();

            for col_name in &column_names {
                if let Some(data) = columns.get(col_name) {
                    if data.is_empty() {
                        continue;
                    }

                    fields.push(Field::new(col_name.as_str(), DataType::Float32, false));
                    arrays.push(Arc::new(Float32Array::from(data.clone())));
                }
            }

            if arrays.is_empty() {
                println!(
                    "    ERROR: No valid arrays for topic '{}', this shouldn't happen!",
                    topic_name
                );
                return Err(anyhow::anyhow!(
                    "Topic '{}' passed validation but has no arrays",
                    topic_name
                ));
            }

            let schema = Arc::new(Schema::new(fields));
            let batch = RecordBatch::try_new(schema.clone(), arrays)?;

            let topic_bytes = topic_name.as_bytes();
            writer.write_all(&(topic_bytes.len() as u32).to_le_bytes())?;
            writer.write_all(topic_bytes)?;

            let mut stream_buffer = Vec::new();
            {
                let mut stream_writer = StreamWriter::try_new(&mut stream_buffer, &schema)?;
                stream_writer.write(&batch)?;
                stream_writer.finish()?;
            }

            writer.write_all(&(stream_buffer.len() as u64).to_le_bytes())?;
            writer.write_all(&stream_buffer)?;
        }

        writer.flush()?;

        Ok(())
    }

    pub fn load_from_arrow<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
        use arrow::ipc::reader::StreamReader;

        self.topics.clear();
        self.start_time = 0.0;

        let file = File::open(&path)?;
        let file_size = file.metadata()?.len();

        let mut reader = BufReader::new(file);

        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        let num_topics = u32::from_le_bytes(buf) as usize;

        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;

        let mut bytes_read = 8u64; // 4 bytes for topic count + 4 bytes for start_time

        for topic_idx in 0..num_topics {
            let mut buf = [0u8; 4];
            reader.read_exact(&mut buf).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to read topic name length for topic {}/{} at byte {}: {}",
                    topic_idx + 1,
                    num_topics,
                    bytes_read,
                    e
                )
            })?;
            bytes_read += 4;
            let name_len = u32::from_le_bytes(buf) as usize;
            let mut name_buf = vec![0u8; name_len];
            reader.read_exact(&mut name_buf).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to read topic name for topic {}/{} at byte {}: {}",
                    topic_idx + 1,
                    num_topics,
                    bytes_read,
                    e
                )
            })?;
            bytes_read += name_len as u64;

            let topic_name = String::from_utf8(name_buf)
                .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in topic name: {}", e))?;

            let mut buf = [0u8; 8];
            reader.read_exact(&mut buf)
            .map_err(|e| anyhow::anyhow!(
                "Failed to read stream size for topic '{}' at byte {}: {}\n\
                 This usually means the previous topic's data was incomplete or the file is truncated.\n\
                 File size: {}, current position: {}, remaining: {}", 
                topic_name, bytes_read, e, file_size, bytes_read, file_size - bytes_read
            ))?;
            bytes_read += 8;
            let stream_size = u64::from_le_bytes(buf) as usize;

            if bytes_read + stream_size as u64 > file_size {
                return Err(anyhow::anyhow!(
                    "Stream size {} would exceed file size. File appears corrupted.\n\
                 Topic: '{}', current position: {}, file size: {}",
                    stream_size,
                    topic_name,
                    bytes_read,
                    file_size
                ));
            }

            let mut stream_data = vec![0u8; stream_size];
            reader.read_exact(&mut stream_data).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to read stream data for topic '{}' (expected {} bytes) at byte {}: {}",
                    topic_name,
                    stream_size,
                    bytes_read,
                    e
                )
            })?;
            bytes_read += stream_size as u64;

            let cursor = std::io::Cursor::new(stream_data);
            let stream_reader = StreamReader::try_new(cursor, None).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create StreamReader for topic '{}': {}",
                    topic_name,
                    e
                )
            })?;

            let mut batch_count = 0;
            for batch_result in stream_reader {
                let batch = batch_result.map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to read batch {} for topic '{}': {}",
                        batch_count,
                        topic_name,
                        e
                    )
                })?;
                let schema = batch.schema();

                let entry = self.topics.entry(topic_name.clone()).or_default();

                for (i, field) in schema.fields().iter().enumerate() {
                    let col_name = field.name();
                    let column = batch.column(i);

                    if let Some(arr) = column.as_any().downcast_ref::<Float32Array>() {
                        let target = entry.entry(col_name.to_string()).or_default();
                        target.extend(arr.values());
                    }
                }
                batch_count += 1;
            }
        }

        if bytes_read != file_size {
            println!("  WARNING: File has {} extra bytes", file_size - bytes_read);
        }

        self.start_time = 0.0;

        Ok(())
    }

    pub fn get_column(&self, topic: &str, col: &str) -> Option<&Vec<f32>> {
        self.topics.get(topic)?.get(col)
    }

    pub fn get_topics(&self) -> Vec<&String> {
        let mut topics: Vec<_> = self.topics.keys().collect();
        topics.sort();
        topics
    }

    pub fn get_columns(&self, topic: &str) -> Vec<&String> {
        if let Some(cols) = self.topics.get(topic) {
            let mut col_names: Vec<_> = cols.keys().collect();
            col_names.sort_by(|a, b| natord::compare(a, b));

            col_names.retain(|&name| name != "timestamp");
            col_names
        } else {
            Vec::new()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.topics.is_empty()
    }
}

impl Default for DataStore {
    fn default() -> Self {
        Self::new()
    }
}
