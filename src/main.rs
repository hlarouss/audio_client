pub mod common;
pub mod wav_client;
pub mod wav_player;

use common::resources::{IP_ADDRESS, FilesList ,WAV_RECORD_FILE_PATH};
use anyhow::Result;

pub const WAV_FILE_SAVE_PATH: &str = "target/received_file.wav";
pub const RECORDED_WAV_FILE_PATH: &str = "target/received_record.wav";

fn delay(duration: u64) {
    std::thread::sleep(std::time::Duration::from_secs(duration));
}

fn main() -> Result<()> {
    println!("Welcome to this wav audio client demo!");
    delay(2);
    println!("Getting the list of wav files from server...");
    delay(2);

    //Get files list
    
    let files_list : FilesList = wav_client::receive_files_list(IP_ADDRESS)?;

    let files_list_string: String = serde_json::to_string(&files_list)?;
    println!("The server has the following files in stock:{}",files_list_string);
    delay(2);

    //Download first file
    println!("Downloading the first wav file from server to the following path: {}",WAV_FILE_SAVE_PATH);
    delay(2);
    
    wav_client::receive_wav_file(IP_ADDRESS, files_list.files[0].to_string(), WAV_FILE_SAVE_PATH)?;

    //Playing content
    println!("Playing wav file content...");
    delay(2);
    wav_player::play_wav_file(WAV_FILE_SAVE_PATH)?;

    //Record request
    println!("Asking to server to perform audio recording...");
    delay(2);
    wav_client::perform_recording(IP_ADDRESS)?;

    //Download record
    println!("Waiting for recording to finish and downloading...");
    delay(2);
    wav_client::receive_wav_file(IP_ADDRESS, WAV_RECORD_FILE_PATH.to_string(), RECORDED_WAV_FILE_PATH)?;

    //Playing content
    println!("Playing record content...");
    delay(2);
    wav_player::play_wav_file(RECORDED_WAV_FILE_PATH)?;

    println!("Demo finished!");
    Ok(())
}
