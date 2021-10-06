use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::StreamInstant;
use image::{ImageBuffer, RgbImage};
use num_complex::Complex;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::{thread, time};

#[derive(Debug)]
struct MyRecData {
    instant: StreamInstant,
    data: Vec<f32>,
}

struct ProcessData {
    duration: time::Duration,
    data: Vec<f32>,
}

const IMAGEX: u32 = 200;
const IMAGEY: u32 = 100;
const STEP: f32 = 20.0;
const START_FREC: f32 = 200.0;

fn generate_buffer() -> RgbImage {
    ImageBuffer::new(IMAGEX, IMAGEY)
}

fn process(p_data: ProcessData, x: u32) {
    let time = p_data.duration.as_nanos() as f32 / (10.0_f32).powf(9.0);
    let data_size = p_data.data.len() as u32;
    let gragh_step = 20 as u32;

    let step = (time as f32 / data_size as f32) * gragh_step as f32;

    let mut i: u32 = 0;
    let mut max = 0.0f64;

    while i < data_size {
        if max < *p_data.data.get(i as usize).unwrap() as f64 {
            max = *p_data.data.get(i as usize).unwrap() as f64;
        }
        i += gragh_step;
    }

    let scale = 240.0f64 / max;

    let mut img_buff: RgbImage = generate_buffer();

    let mut sum_vec: Vec<Complex<f32>> = vec![];

    for j in 0..(IMAGEX + 1) {
        let test_frec: f32 = j as f32 * STEP + START_FREC;
        i = 0;
        while i < data_size {
            let to_sum = Complex::from_polar(
                *p_data.data.get(i as usize).unwrap() as f32 * scale as f32,
                -2.0 * std::f32::consts::PI * test_frec * (i as f32 / gragh_step as f32) * step,
            );
            if sum_vec.get(j as usize).is_none() {
                sum_vec.push(to_sum);
            } else {
                let a = *sum_vec.get(j as usize).unwrap();
                let _ = std::mem::replace(&mut sum_vec[j as usize], a + to_sum);
            }
            i += gragh_step;
        }
    }
    let mut max = 0.0f32;
    let mut average = 0.0f32;
    let mut possible = 0.0f32;
    for i in 0..IMAGEX {
        let v: Complex<f32> = *sum_vec.get(i as usize).unwrap();
        if !v.is_nan() {
            let v = (v.re.powf(2.0) + v.im.powf(2.0)).sqrt();
            if max < v {
                max = v;
                possible = START_FREC + STEP * i as f32;
            }
            average += v;
        }
    }
    average = IMAGEY as f32 - (average / IMAGEX as f32) / max * (IMAGEY - 10) as f32;
    for i in 0..IMAGEX {
        let v: Complex<f32> = *sum_vec.get(i as usize).unwrap();
        if !v.is_nan() {
            let j = IMAGEY as f32
                - ((v.re.powf(2.0) + v.im.powf(2.0)).sqrt() / max) * (IMAGEY - 10) as f32;
            *img_buff.get_pixel_mut(i as u32, j.floor() as u32) = image::Rgb([255, 0, 255]);
            *img_buff.get_pixel_mut(i as u32, average as u32) = image::Rgb([255, 0, 0]);
        }
    }

    println!("Processed {} {}", x, possible);

    img_buff.save(format!("run/process{}.png", x)).unwrap();
}

//fn main1() {
////generateBuffer();
//let complex = Complex::from_polar(1.0, std::f32::consts::FRAC_PI_2);
//println!("im: {}, re: {}", complex.im, complex.re);
//}

fn main() {
    println!("Audio Test:");

    let host = cpal::default_host();

    let rec_data: Arc<Mutex<Vec<MyRecData>>> = Arc::new(Mutex::new(Vec::new()));
    let rec_data_c = rec_data.clone();

    let (m_t, m_r) = mpsc::channel();

    println!("Geting audio input:");

    let device = host
        .default_input_device()
        .expect("no output device available!");

    let mut supported_configs_range = device
        .supported_input_configs()
        .expect("Error while geting the configs");
    let supported_config = supported_configs_range
        .next()
        .expect("no supported config")
        .with_max_sample_rate()
        .into();

    println!("Got config: {:?}", supported_config);

    let stream = device
        .build_input_stream(
            &supported_config,
            move |data: &[f32], input_info: &cpal::InputCallbackInfo| {
                //            println!("Data:\n Input info:{:?}",  input_info.timestamp().capture);
                let mdata = MyRecData {
                    instant: input_info.timestamp().capture,
                    data: data.to_vec(),
                };
                rec_data_c.lock().ok().unwrap().insert(0, mdata);
                ()
            },
            move |err| {
                println!("Error: {:?}", err);
            },
        )
        .expect("Failed to build stream");

    stream.play().expect("faild to play");

    let t = thread::spawn(move || {
        let mut run = true;
        let mut op = None;
        let mut len = None;
        let mut last = None;
        let mut x = 0;
        while run {
            let mut v = rec_data.lock().ok().unwrap();
            let md = m_r.try_recv();
            let vlen = v.len();
            if len.is_none() {
                println!("Starting len: {}", vlen);
                len = Some(vlen);
            }
            if len.unwrap() != vlen {
                //                println!("Cur len: {}", v.len());
                len = Some(vlen);
            }
            if md.is_ok() {
                println!("got stop");
                op = Some(md.ok().unwrap());
            }
            if op.is_some() {
                if vlen <= 0 {
                    println!("stoping process");
                    run = op.unwrap();
                } else {
                    //                    println!("Queue:{}", vlen);
                }
            }
            if vlen > 0 {
                x += 1;
                //                println!("got one id: {}", x);
                let mdata: MyRecData = v.pop().unwrap();
                drop(v);
                if last.is_none() {
                    last = Some(mdata.instant);
                } else {
                    let instant = mdata.instant.duration_since(&last.unwrap()).unwrap();
                    process(
                        ProcessData {
                            duration: instant,
                            data: mdata.data,
                        },
                        x,
                    );
                    last = Some(mdata.instant);
                    //                    println!("time:{:?}", instant);
                }
            }
        }
        println!("Stoped processing");
    });

    thread::sleep(time::Duration::from_secs(30));

    stream.pause().expect("faild to close");

    println!("stoping");
    m_t.send(false).ok();
    t.join().unwrap();
}
