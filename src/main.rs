use std::io::prelude::*;
use std::net::TcpStream;
use std::io;
use std::path;
use std::fs;
use hound;

//Constants
const SAMPLE_RATE:u32 = 48000;

/*
"SEND_WAV_FILE"
"RECORD_AND_SEND"
*/

fn bytes_to_audio(audio_bytes_buffer:[u8;(i16::MAX as usize *2) as usize]) -> [i16;i16::MAX as usize] {
    let mut audio_buffer : [i16;i16::MAX as usize] = [0;i16::MAX as usize];

    for i in 0..audio_buffer.len() {
        audio_buffer[i] = ((audio_bytes_buffer[i*2] as i16) << 8) | (audio_bytes_buffer[i*2 + 1] as i16);
    }

    return audio_buffer;
}

fn tcp_receive_chunk(mut stream: &TcpStream) -> std::io::Result<[i16;i16::MAX as usize]>{
    let mut audio_bytes_buffer: [u8;(i16::MAX as usize *2) as usize] = [0;(i16::MAX as usize *2) as usize];

    stream.read_exact(&mut audio_bytes_buffer)?;

    Ok(bytes_to_audio(audio_bytes_buffer))
}

fn tcp_write_wav_file<P: AsRef<path::Path>>(stream: &TcpStream, filename: P, samples_amount: i64) -> io::Result<()> {

    let mut samples_left : i64 = samples_amount;

    let spec: hound::WavSpec = hound::WavSpec {
        channels: 2,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer: hound::WavWriter<io::BufWriter<fs::File>> = hound::WavWriter::create(filename, spec).unwrap();

    while  samples_left - (i16::MAX as i64) > 0 {
        let audio_buffer: [i16;i16::MAX as usize] = tcp_receive_chunk(&stream)?;

        for val in audio_buffer.into_iter() {
            writer.write_sample(val).unwrap();
        }

        samples_left = samples_left - (i16::MAX as i64);
    }
    //Process last batch if applicable
    if samples_left > 0 {
        let audio_buffer: [i16;i16::MAX as usize] = tcp_receive_chunk(&stream)?;

        for i in 0..samples_left as usize {
            writer.write_sample(audio_buffer[i]).unwrap();
        }
    }

    writer.finalize().unwrap();

    Ok(())
}

fn main() -> std::io::Result<()> {
    let mut stream: TcpStream = TcpStream::connect("127.0.0.1:8000")?;

    let mut samples_amount_bytes: [u8;4] = [0;4];
    stream.write(b"SEND_WAV_FILE")?;
    stream.read_exact(&mut samples_amount_bytes)?;

    let samples_amount: u32 =   ((samples_amount_bytes[0] as u32) << 24) | 
                                ((samples_amount_bytes[1] as u32) << 16) | 
                                ((samples_amount_bytes[2] as u32) << 8) | 
                                ((samples_amount_bytes[3] as u32));

    println!("Amount of samples: {}", samples_amount);

    
    tcp_write_wav_file(&stream,"target/received_audio.wav",samples_amount as i64)?;

    Ok(())
} // the stream is closed here
