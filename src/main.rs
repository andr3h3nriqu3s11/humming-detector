use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use cpal::StreamInstant;
use std::{thread, time};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use num_complex::Complex;
use image::{ImageBuffer, RgbImage};

#[derive(Debug)]
struct MyRecData {
    instant: StreamInstant,
    data: Vec<f32>,
}

struct ProcessData {
    duration: time::Duration,
    data: Vec<f32>,
}

const IMAGEX: u32 = 500;
const IMAGEY: u32 = 500;

fn generate_buffer () -> RgbImage {
   
    let mut imgbuf: RgbImage = ImageBuffer::new(IMAGEX, IMAGEY);

    for i in 0..IMAGEX {
        *imgbuf.get_pixel_mut(i, 250) = image::Rgb([255,255,255]);
    }
    
    //imgbuf.save("test.png").unwrap();
    imgbuf
}

fn process(p_data: ProcessData, x: u32) {
    let time = p_data.duration.as_nanos() as f32 / (10.0_f32).powf(9.0);

    let mut sum: Complex<f32> = Complex::new(0.0, 0.0);

    let test_frec = 440.0f32;
    
    let data_size = p_data.data.len() as u32;
    let gragh_step = (data_size as f64 / (IMAGEX as f64 - 1.0f64)).floor() as u32;

    println!("gs:{}", gragh_step);
    
    let step = (time as f32 / data_size as f32) * gragh_step as f32;
    
    let mut i: u32 = 0;
    let mut max = 0.0f64;

    while i < data_size && i/gragh_step < IMAGEX - 1 {
        if max < *p_data.data.get(i as usize).unwrap() as f64 {
            max = *p_data.data.get(i as usize).unwrap() as f64;
        }
        i += gragh_step;
    }
    
    let scale = 240.0f64 / max;

    let mut img_buff: RgbImage = generate_buffer();

    i = 0;

    while i < data_size && i/gragh_step < IMAGEX - 1 {
        let y =  (250.0f64 - (*p_data.data.get(i as usize).unwrap() as f64 * scale).floor()) as u32;
        let to_sum = Complex::from_polar(*p_data.data.get(i as usize).unwrap() as f32 * scale as f32, - 2.0 * std::f32::consts::PI * test_frec * ( i as f32/gragh_step as f32) *  step);
        sum += to_sum;
        if y < 500 && y > 0 { 
            *img_buff.get_pixel_mut((i/gragh_step) as u32, y ) = image::Rgb([0, 255, 255]);
        }
        i += gragh_step;
    }
//    sum = sum.scale(1.0f32 / (data_size as f32 /gragh_step as f32));
    if !sum.re.is_nan() && !sum.im.is_nan() {
        println!("center: {}", (sum.im.powf(2.0) + sum.im.powf(2.0)).sqrt() )
    }
    let x1 = sum.re + (IMAGEX/2) as f32;
    let y1 = sum.im + 250.0;
    if x1 > 0.0 && x1 < IMAGEX as f32 && y1 > 0.0 && y1 < 500.0 {
        *img_buff.get_pixel_mut( x1 as u32, y1 as u32 ) = image::Rgb([255, 255, 255]);
        //println!("center");
    }

    println!("Processed {}", x);
    
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

    let device = host.default_input_device().expect("no output device available!");

    let mut supported_configs_range = device.supported_input_configs().expect("Error while geting the configs");
    let supported_config = supported_configs_range.next().expect("no supported config").with_max_sample_rate().into();

    println!("Got config: {:?}", supported_config);
        
    let stream = device.build_input_stream(&supported_config,
        move |data: &[f32], input_info: &cpal::InputCallbackInfo| {
//            println!("Data:\n Input info:{:?}",  input_info.timestamp().capture);
            let mdata = MyRecData {instant: input_info.timestamp().capture, data: data.to_vec()};
            rec_data_c.lock().ok().unwrap().insert(0, mdata);
            ()
        },
        move |err| {
            println!("Error: {:?}", err);
    }).expect("Failed to build stream");

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
                    process(ProcessData {duration: instant, data: mdata.data}, x);
                    last = Some(mdata.instant);
//                    println!("time:{:?}", instant);
                }               
            }
        }  
        println!("Stoped processing");
    });

    thread::sleep(time::Duration::from_secs(2));

    stream.pause().expect("faild to close");

    println!("stoping");
    m_t.send(false).ok();
    t.join().unwrap();    

}
