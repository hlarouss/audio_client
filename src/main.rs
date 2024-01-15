use std::io::prelude::*;
use std::net::TcpStream;
use hound;

//Constants
const SAMPLE_RATE:u32 = 44100;
const DURATION:u32 = 3;
const SAMPLES_AMOUNT:usize = (SAMPLE_RATE * DURATION) as usize;

fn main() -> std::io::Result<()> {
    let mut stream = TcpStream::connect("127.0.0.1:8000")?;

    let mut audio_array : [i16;SAMPLES_AMOUNT] = [0;SAMPLES_AMOUNT];
    let mut audio_array_bytes : [u8;SAMPLES_AMOUNT*2] = [0;SAMPLES_AMOUNT*2];

    stream.write(&[1])?;
    stream.read_exact(&mut audio_array_bytes)?;

    for i in 0..SAMPLES_AMOUNT {
        audio_array[i] = ((audio_array_bytes[i*2] as i16) << 8) | (audio_array_bytes[i*2 + 1] as i16);
    }

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create("target/received_sine.wav", spec).unwrap();
    for val in audio_array.into_iter() {
        writer.write_sample(val).unwrap();
    }
    writer.finalize().unwrap();

    Ok(())
} // the stream is closed here
