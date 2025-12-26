use arrow::record_batch::RecordBatch;
use crossbeam_channel::Sender;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Cursor;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

#[derive(Debug)]
pub enum DataMessage {
    Metadata(TimelineRange),
    NewBatch(String, RecordBatch),
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct TimelineRange {
    pub min_timestamp: Option<i64>,
    pub max_timestamp: Option<i64>,
}

#[derive(Deserialize, Debug)]
struct PacketMetadata {
    #[allow(dead_code)]
    parameters: HashMap<String, serde_json::Value>,
    #[allow(dead_code)]
    version_info: HashMap<String, String>,
    table_count: usize,
    #[allow(dead_code)]
    table_names: Vec<String>,
    timeline_range: TimelineRange,
}

pub fn start_tcp_server(sender: Sender<DataMessage>, ctx: egui::Context) {
    tokio::spawn(async move {
        let listener = TcpListener::bind("127.0.0.1:9999")
            .await
            .expect("Failed to bind TCP port 9999");

        println!("TCP Receiver listening on 127.0.0.1:9999");

        loop {
            match listener.accept().await {
                Ok((mut socket, addr)) => {
                    println!("New connection from: {}", addr);

                    if let Err(e) = handle_connection(&mut socket, &sender, &ctx).await {
                        eprintln!("Error handling connection: {}", e);
                    }

                    println!("Connection closed");
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }
    });
}

async fn handle_connection(
    socket: &mut tokio::net::TcpStream,
    sender: &Sender<DataMessage>,
    ctx: &egui::Context,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut len_buf = [0u8; 4];
    socket.read_exact(&mut len_buf).await?;
    let meta_len = u32::from_le_bytes(len_buf) as usize;

    let mut meta_json = vec![0u8; meta_len];
    socket.read_exact(&mut meta_json).await?;

    let metadata: PacketMetadata = serde_json::from_slice(&meta_json)?;
    println!("Received metadata: {} tables", metadata.table_count);

    sender
        .send(DataMessage::Metadata(metadata.timeline_range))
        .ok();

    ctx.request_repaint();

    for _i in 0..metadata.table_count {
        socket.read_exact(&mut len_buf).await?;
        let name_len = u32::from_le_bytes(len_buf) as usize;

        let mut name_buf = vec![0u8; name_len];
        socket.read_exact(&mut name_buf).await?;
        let table_name = String::from_utf8_lossy(&name_buf).to_string();

        let mut size_buf = [0u8; 8];
        socket.read_exact(&mut size_buf).await?;
        let table_size = u64::from_le_bytes(size_buf) as usize;

        let mut arrow_data = vec![0u8; table_size];
        socket.read_exact(&mut arrow_data).await?;

        let cursor = Cursor::new(arrow_data);
        match arrow::ipc::reader::StreamReader::try_new(cursor, None) {
            Ok(reader) => {
                for batch_result in reader {
                    match batch_result {
                        Ok(batch) => {
                            sender
                                .send(DataMessage::NewBatch(table_name.clone(), batch))
                                .ok();

                            ctx.request_repaint();
                        }
                        Err(e) => {
                            eprintln!("Error reading batch from '{}': {}", table_name, e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Arrow IPC parse error for '{}': {}", table_name, e);
            }
        }
    }

    println!("Finished processing all tables");
    Ok(())
}
