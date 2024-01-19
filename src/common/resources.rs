use serde::{Deserialize, Serialize};

pub const BYTE_BUFFER_SIZE: usize = (u16::MAX as usize) + (1 as usize);
pub const I8_SIZE : usize = 1;
pub const I16_SIZE : usize = 2;
pub const I32_SIZE : usize = 4;
pub const F32_SIZE : usize = 4;

pub const IP_ADDRESS: &str = "127.0.0.1:8000";
pub const WAV_FILES_FOLDER_PATH : &str = "wav_resources";
pub const WAV_RECORD_FILE_PATH: &str = "wav_resources/record.wav";
pub const RECORD_DURATION: u64 = 10;

#[derive(Serialize, Deserialize)]
pub enum Command {
    PerformRecording,
    SendFilesList,
    SendFileSpecs,
    SendFile,
}

#[derive(Serialize, Deserialize)]
pub enum SampleFormat {
    Float,
    Int,
}

#[derive(Serialize, Deserialize)]
pub struct WavFileSpecs {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub sample_format: SampleFormat,
    pub samples_amount: u32,
}

#[derive(Serialize, Deserialize)]
pub struct ClientRequest {
    pub command: Command,
    pub filename: String,
    pub record_duration: u64,
}

#[derive(Serialize, Deserialize)]
pub struct FilesList {
    pub files: Vec<String>,
}