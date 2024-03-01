use clap::Parser;
use gilrs::{Gilrs, Button, Event, EventType};
// use log::LevelFilter;
// use log4rs::append::file::FileAppender;
// use log4rs::encode::pattern::PatternEncoder;
// use log4rs::config::{Appender, Config, Root};
use log::debug;
use log::info;
use log::warn;
use std::io;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::watch;
// use cgmath::Vector2;
// use rand::distributions::{Normal, IndependentSample};
use std::f32::consts::PI;
// use cgmath::Vector2;
// use rand_distr::{Normal, Distribution};
// use rand;
// use cgmath::InnerSpace;
use num::complex::Complex32;

use channel_models::{calculate_paths_freespace, calculate_paths_two_ray, calculate_paths_ce2r, calculate_paths_9ray_suburban, distance, FREQUENCY, FAR_FIELD_DISTANCE};

const MAX_TAPS: usize = 41;

const TAP_VALUE_NO_LOSS: Complex32 = Complex32::new(32767.0, 0.0);
// const MAGIC_SCALING_COEFF: f32 = 140.5;
const MAGIC_SCALING_COEFF: f32 = 30000.0;
const TAP_VALUE_MAX: i16 = 32760;
const TAP_VALUE_MIN: i16 = -32760;

#[derive(Parser, Debug)]
struct Args {
    /// UDP port to receive position updates
    #[clap(short, long, default_value_t = 1337)]
    local_udp_port: u32,
    /// UDP port to receive position updates
    #[clap(short, long, default_value_t = 1341)]
    model_selection_udp_port: u32,
    /// UDP port of channel emulator
    #[clap(short, long, default_value_t = 1338)]
    chanem_port: u32,
    /// Sample Rate
    #[clap(long, default_value_t = 200e6)]
    sample_rate: f64,
}

fn loss_to_tap_value(loss_linear: f32, phase: Complex32, magic_scaling_coeff: f32) -> Complex32 {
    if loss_linear == 0. {
        TAP_VALUE_NO_LOSS * phase
    } else {
        (TAP_VALUE_NO_LOSS / loss_linear) * magic_scaling_coeff
    }
}

fn convert_paths_to_taps(paths: Vec<(f32, f32, f32)>, sample_rate: f32, magic_scaling_coeff: f32) -> [i16; MAX_TAPS * 2] {
    let delay_per_tap = 1. / sample_rate;
    let mut taps_complex = [Complex32::new(0., 0.); MAX_TAPS];
    for (loss_linear, delay, additional_phase_shift) in paths.into_iter() {
        let tap_index = (delay / delay_per_tap).floor() as usize;
        if tap_index > 2 {
            println!("tap index: {}", tap_index);
        }
        if tap_index < MAX_TAPS {
            let phase_offset = 2. * PI * delay * FREQUENCY + additional_phase_shift;
            let phase = Complex32::from_polar(1., phase_offset);
            let mpc = loss_to_tap_value(loss_linear, phase, magic_scaling_coeff);
            taps_complex[tap_index] += mpc;
        };
    }
    let mut taps = [0_i16; MAX_TAPS * 2];
    for (i, complex_tap) in taps_complex.iter().enumerate() {
        taps[i] = (complex_tap.re as i16).clamp(TAP_VALUE_MIN, TAP_VALUE_MAX);
        taps[i + MAX_TAPS] = (complex_tap.im as i16).clamp(TAP_VALUE_MIN, TAP_VALUE_MAX);
    }
    taps
}

// fn calculate_taps_two_segment_log_dist(
//     x: f32, y: f32, z: f32, r_rad: f32, p_rad: f32, y_rad: f32
// ) -> [i16; MAX_TAPS * 2] {
//     let mut taps = [0i16; MAX_TAPS * 2];
//
//     let dist = distance(x.copy(), y.copy(), z);
//     let theta = (distance(x.copy(), y.copy(), STATION_Z) / dist.copy()).atan();
//
//     let to_origin = Vector2(x - STATION_X, y - STATION_Y);
//     let yaw_vec = Vector2(y_rad.cos(), y_rad.sin());
//     let direction_of_travel = if to_origin.dot(yaw_vec) > 0 {-1} else {1};
//
//     if theta < 5.0_f64.to_radians() {
//         let normal = Normal::new(0.0, 3.3);
//         let x = normal.ind_sample(&mut rand::thread_rng());
//         let path_loss = 116.4 + 10 * 1.6 * (dist / 3000).log10() + X_s + direction_of_travel*3.0;
//     }
//     else {
//         let normal = Normal::new(0.0, 3.0);
//         let x = normal.ind_sample(&mut rand::thread_rng());
//         let path_loss = 123.5 + 10 * 1.8 * (dist / 6600).log10() + X_l + direction_of_travel*1.7;
//     }
//     let tap = 10 ** (REFERENCE_SIGNAL_LEVEL_NO_LOSS_DBM - path_loss);
//     let tap = tap.clamp(0.0, 10000.0);
//
//     info!("distance {:?}, yaw {:?}rad, path-loss {:?}dB", dist, y_rad, path_loss);
//
//     taps[0] = tap as i16;
//     taps[MAX_TAPS] = tap as i16;
//     //taps[0] = 8000;
//     taps
// }

#[derive(Debug)]
enum Ev {
    ModeManual(f32),
    ModeAutomaticFreeSpace,
    ModeAutomaticFlatEarthTwoRay,
    ModeAutomaticCurvedEarthTwoRay,
    ModeAutomaticNineRay,
    Value(f32, f32, f32, f32, f32, f32),
    ScalingCoeff(f32),
}

const NUM_MODES: usize = 5;
const MODEL_INDEX_AUTOMATIC_FREE_SPACE: usize = 0;
const MODEL_INDEX_AUTOMATIC_FLAT_EARTH_TWO_RAY: usize = 1;
const MODEL_INDEX_AUTOMATIC_CURVED_EARTH_TWO_RAY: usize = 2;
const MODEL_INDEX_AUTOMATIC_NINE_RAY: usize = 3;
const MODEL_INDEX_MANUAL: usize = 4;


#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );
    // let logfile = FileAppender::builder()
    //     .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
    //     .build("/tmp/log_chanem.txt").unwrap();
    //
    // let config = Config::builder()
    //     .appender(Appender::builder().build("logfile", Box::new(logfile)))
    //     .build(Root::builder()
    //         .appender("logfile")
    //         .build(LevelFilter::Info)).unwrap();
    //
    // log4rs::init_config(config).unwrap();

    let args = Args::parse();
    info!("Args: {:?}", args);

    let (tx, mut rx) = unbounded_channel();
    let my_tx = tx.clone();
    let my_tx_1 = tx.clone();

    let (to_gui_udp_handler_tx, mut to_gui_udp_handler_rx) = unbounded_channel();
    let to_gui_udp_handler_tx_1 = to_gui_udp_handler_tx.clone();
    let to_gui_udp_handler_tx_2 = to_gui_udp_handler_tx.clone();

    let (mode_channel_gui_to_gamepad_tx, mode_channel_gui_to_gamepad_rx) = watch::channel(MODEL_INDEX_AUTOMATIC_FREE_SPACE);

    std::thread::spawn(move || {
        let mut current_value = 50.0_f32;
        let mut gilrs = Gilrs::new().unwrap();
        let gamepad = gilrs.gamepads().next().map(|(_, b)| b);
        if let Some(pad) = gamepad {
            info!("{} is {:?}", pad.name(), pad.power_info());
            loop {
                while let Some(Event { event, .. }) = gilrs.next_event() {
                    let mut pl_model_index = *mode_channel_gui_to_gamepad_rx.borrow();
                    let mut send = false;
                    let mut send_control_event = false;
                    let mut control_event = b"E00";
                    if matches!(event, EventType::ButtonReleased(Button::East, _)) {
                        pl_model_index += 1;
                        pl_model_index = pl_model_index % NUM_MODES;
                        if pl_model_index == MODEL_INDEX_AUTOMATIC_FREE_SPACE {
                            info!("mode automatic - Free-Space PL");
                            my_tx.send(Ev::ModeAutomaticFreeSpace).unwrap();
                        } else if pl_model_index == MODEL_INDEX_AUTOMATIC_FLAT_EARTH_TWO_RAY {
                            info!("mode automatic - Flat-Earth Two-Ray PL");
                            my_tx.send(Ev::ModeAutomaticFlatEarthTwoRay).unwrap();
                        } else if pl_model_index == MODEL_INDEX_AUTOMATIC_CURVED_EARTH_TWO_RAY {
                            info!("mode automatic - Curved-Earth Two-Ray PL");
                            my_tx.send(Ev::ModeAutomaticCurvedEarthTwoRay).unwrap();
                        } else if pl_model_index == MODEL_INDEX_AUTOMATIC_NINE_RAY {
                            info!("mode automatic - Curved-Earth Two-Ray PL");
                            my_tx.send(Ev::ModeAutomaticNineRay).unwrap();
                        } else {
                            info!("mode manual - {}dB", current_value);
                            my_tx.send(Ev::ModeManual(current_value)).unwrap();
                        }
                        send=true;
                    } else if matches!(event, EventType::ButtonReleased(Button::DPadDown, _)) {
                        if pl_model_index == MODEL_INDEX_MANUAL {
                            current_value += 5.0;
                            current_value = current_value.clamp(0.0, 120.0);
                            info!("mode manual - {}dB", current_value);
                            my_tx.send(Ev::ModeManual(current_value)).unwrap();
                        }
                        send=true;
                    } else if matches!(event, EventType::ButtonReleased(Button::DPadUp, _)) {
                        if pl_model_index == MODEL_INDEX_MANUAL {
                            current_value -= 5.0;
                            current_value = current_value.clamp(0.0, 120.0);
                            info!("mode manual - {}dB", current_value);
                            my_tx.send(Ev::ModeManual(current_value)).unwrap();
                        }
                        send=true;
                    } else if matches!(event, EventType::ButtonPressed(Button::RightTrigger2, _)) {
                        send_control_event = true;
                        control_event = b"ETR";
                    } else if matches!(event, EventType::ButtonPressed(Button::LeftTrigger2, _)) {
                        send_control_event = true;
                        control_event = b"ETL";
                    } else if matches!(event, EventType::ButtonReleased(Button::West, _)) {
                        send_control_event = true;
                        control_event = b"EAW";
                    } else if matches!(event, EventType::ButtonReleased(Button::South, _)) {
                        send_control_event = true;
                        control_event = b"EAS";
                    } else if matches!(event, EventType::ButtonReleased(Button::North, _)) {
                        send_control_event = true;
                        control_event = b"EAN";
                    }
                    if send {
                        let mut send_buf = current_value.to_be_bytes().to_vec();
                        send_buf.insert(0_usize, pl_model_index as u8);
                        // prepend 'M' as message type to distinguish between [P]osition, [T]aps, and [M]ode
                        send_buf.insert(0_usize, b'M');
                        to_gui_udp_handler_tx.send(send_buf).unwrap();
                    }
                    if send_control_event {
                        to_gui_udp_handler_tx.send(control_event.to_vec()).unwrap();
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    });

    let sock_tx = UdpSocket::bind("0.0.0.0:0").await?;
    sock_tx
        .connect(format!("127.0.0.1:{}", args.chanem_port))
        .await
        .unwrap();

    tokio::spawn(async move {
        info!("spawning position update receiver, listening on port {}", args.local_udp_port);
        let sock = UdpSocket::bind(format!("0.0.0.0:{}", args.local_udp_port)).await.unwrap();
        let mut buf = [0; 2048];
        loop {
            let (len, addr) = sock.recv_from(&mut buf).await.unwrap();
            debug!("{:?} bytes received from {:?}", len, addr);

            if len == 24 {
                let x = f32::from_be_bytes(buf[0..4].try_into().unwrap());
                let y = f32::from_be_bytes(buf[4..8].try_into().unwrap());
                let z = f32::from_be_bytes(buf[8..12].try_into().unwrap());
                let r_rad = f32::from_be_bytes(buf[12..16].try_into().unwrap());
                let p_rad = f32::from_be_bytes(buf[16..20].try_into().unwrap());
                let y_rad = f32::from_be_bytes(buf[20..24].try_into().unwrap());

                tx.send(Ev::Value(x, y, z, r_rad, p_rad, y_rad)).unwrap();
                debug!("received ([{}, {}, {}], [{}, {}, {}])", x, y, z, r_rad, p_rad, y_rad);

                let mut send_buf = buf.to_vec();
                // prepend 'P' as message type to distinguish between [P]osition, [T]aps, and [M]ode
                send_buf.insert(0_usize, b'P');
                to_gui_udp_handler_tx_1.send(send_buf).unwrap();  // TODO
            }
            else {
                // erroneous message contains: b'PowerFolder node: [1337]-[AUTJpBd5EcTPnEtSPDkZ]\x00'
                // some external program (PowerFolder, probably connected to HessenBox on some PC in the local network) also uses port 1337 -> ignore this specific message
                // there might still arrive other malformed packages -> log for further inspection
                let known_malformed_msg_prefix: [u8; 24] = [80, 111, 119, 101, 114, 70, 111, 108, 100, 101, 114, 32, 110, 111, 100, 101, 58, 32, 91, 49, 51, 51, 55, 93];
                if len > 24 && buf[..24] == known_malformed_msg_prefix {
                }
                else {
                    info!("WARNING 001: received {:?}", &buf);
                }
            }
        }
    });

    // udp receiver from gui
    tokio::spawn(async move {
        let sock = UdpSocket::bind(format!("0.0.0.0:{}", args.model_selection_udp_port)).await.unwrap();
        let mut buf = [0; 1024];
        loop {
            let (len, addr) = sock.recv_from(&mut buf).await.unwrap();
            debug!("{:?} bytes received from {:?}", len, addr);

            if len == 1 {
                // let mut received = std::str::from_utf8(&buf[0..1]).unwrap().trim();
                // let new_pl_model_index = received.parse::<usize>().unwrap();
                let new_pl_model_index = buf[0] as usize;
                if new_pl_model_index == MODEL_INDEX_AUTOMATIC_FREE_SPACE {
                    info!("mode automatic - Free-Space PL");
                    my_tx_1.send(Ev::ModeAutomaticFreeSpace).unwrap();
                } else if new_pl_model_index == MODEL_INDEX_AUTOMATIC_FLAT_EARTH_TWO_RAY {
                    info!("mode automatic - Flat-Earth Two-Ray PL");
                    my_tx_1.send(Ev::ModeAutomaticFlatEarthTwoRay).unwrap();
                } else if new_pl_model_index == MODEL_INDEX_AUTOMATIC_CURVED_EARTH_TWO_RAY {
                    info!("mode automatic - Curved-Earth Two-Ray PL");
                    my_tx_1.send(Ev::ModeAutomaticCurvedEarthTwoRay).unwrap();
                } else if new_pl_model_index == MODEL_INDEX_AUTOMATIC_NINE_RAY {
                    info!("mode automatic - Curved-Earth Two-Ray PL");
                    my_tx_1.send(Ev::ModeAutomaticNineRay).unwrap();
                } else {
                    info!("mode manual {}", -1.);
                    my_tx_1.send(Ev::ModeManual(-1.)).unwrap();
                }
                info!("received new pl_model_index: {}", new_pl_model_index);
            } else if len == 4 {
                let magic_scaling_coeff_tmp = f32::from_be_bytes(buf[0..4].try_into().unwrap());
                info!("received new magic scaling coefficient: {}", magic_scaling_coeff_tmp);
                my_tx_1.send(Ev::ScalingCoeff(magic_scaling_coeff_tmp)).unwrap();
            } else {
                warn!("received invalid data from GUI: was not of length 1 (u8) or 4 (f32).")
            }
        }
    });

    // udp sender to gui
    tokio::spawn(async move {
        let sock_tx_to_gui = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        sock_tx_to_gui
            // forward to Host (has .1 address of every docker compose network)
            .connect("172.18.0.1:1342")
            .await
            .unwrap();
        let mut to_gui_udp_handler_rx = to_gui_udp_handler_rx;
        loop {
            if let Some(payload) = to_gui_udp_handler_rx.recv().await {
                match sock_tx_to_gui.send(&payload).await {
                    Ok(_) => {
                        debug!("success sending to GUI.")
                    }
                    Err(e) => {
                        warn!("error sending position update to GUI ({:?})", e);
                    }
                };
            }
        }
    });

    let mut taps = [0i16; MAX_TAPS * 2];
    let mut pl_model_index = MODEL_INDEX_AUTOMATIC_FREE_SPACE;
    let mut magic_scaling_coeff: f32 = MAGIC_SCALING_COEFF;
    let mut last_manual = 50.0_f32;
    loop {
        let mut send = false;
        if let Some(e) = rx.recv().await {
            match e {
                Ev::ModeAutomaticFreeSpace => {
                    pl_model_index = MODEL_INDEX_AUTOMATIC_FREE_SPACE;
                    if let Err(e) = mode_channel_gui_to_gamepad_tx.send(MODEL_INDEX_AUTOMATIC_FREE_SPACE) {
                        warn!("error sending PL model index to gui ({:?})", e);
                    }
                },
                Ev::ModeAutomaticFlatEarthTwoRay => {
                    pl_model_index = MODEL_INDEX_AUTOMATIC_FLAT_EARTH_TWO_RAY;
                    if let Err(e) = mode_channel_gui_to_gamepad_tx.send(MODEL_INDEX_AUTOMATIC_FLAT_EARTH_TWO_RAY) {
                        warn!("error sending PL model index to gui ({:?})", e);
                    }
                },
                Ev::ModeAutomaticCurvedEarthTwoRay => {
                    pl_model_index = MODEL_INDEX_AUTOMATIC_CURVED_EARTH_TWO_RAY;
                    if let Err(e) = mode_channel_gui_to_gamepad_tx.send(MODEL_INDEX_AUTOMATIC_CURVED_EARTH_TWO_RAY) {
                        warn!("error sending PL model index to gui ({:?})", e);
                    }
                },
                Ev::ModeAutomaticNineRay => {
                    pl_model_index = MODEL_INDEX_AUTOMATIC_NINE_RAY;
                    if let Err(e) = mode_channel_gui_to_gamepad_tx.send(MODEL_INDEX_AUTOMATIC_NINE_RAY) {
                        warn!("error sending PL model index to gui ({:?})", e);
                    }
                },
                Ev::ModeManual(v) => {
                    pl_model_index = MODEL_INDEX_MANUAL;
                    if let Err(e) = mode_channel_gui_to_gamepad_tx.send(MODEL_INDEX_MANUAL) {
                        warn!("error sending PL model index to gui ({:?})", e);
                    }
                    if v >= 0. {
                        last_manual = v;
                    }
                    taps.fill(0);
                    let tap: Complex32 = TAP_VALUE_NO_LOSS / 10.0_f32.powf(last_manual / 20.0_f32) * magic_scaling_coeff;
                    let tap = (tap.re as i16).clamp(TAP_VALUE_MIN, TAP_VALUE_MAX);
                    taps[0] = tap;
                    // taps[MAX_TAPS] = tap;
                    send = true;
                },
                Ev::Value(x, y, z, _r_rad, _p_rad, _y_rad) => {
                    if !(pl_model_index == MODEL_INDEX_MANUAL) {
                        send = true;
                        let dist = distance(x, y, z);
                        let paths: Vec<(f32, f32, f32)> = if dist < FAR_FIELD_DISTANCE {
                            vec![(1., 0., 0.)]
                        } else {
                            if pl_model_index == MODEL_INDEX_AUTOMATIC_FLAT_EARTH_TWO_RAY {
                                calculate_paths_two_ray(x, y, z)
                            } else if pl_model_index == MODEL_INDEX_AUTOMATIC_CURVED_EARTH_TWO_RAY {
                                calculate_paths_ce2r(x, y, z)
                            } else if pl_model_index == MODEL_INDEX_AUTOMATIC_NINE_RAY {
                                calculate_paths_9ray_suburban(x, y, z)
                            } else if pl_model_index == MODEL_INDEX_AUTOMATIC_FREE_SPACE {
                                calculate_paths_freespace(x, y, z)
                            } else {
                                panic!("invalid pl_model_index: {}", pl_model_index)
                            }
                        };
                        // println!("{:?}", &paths);
                        taps = convert_paths_to_taps(paths, args.sample_rate as f32, magic_scaling_coeff);
                    }
                },
                Ev::ScalingCoeff(v) => {
                    magic_scaling_coeff = v;
                }
            }

            if send {
                match sock_tx
                    .send(
                        &taps
                            .iter()
                            .flat_map(|v| v.to_be_bytes())
                            .collect::<Vec<u8>>(),
                    )
                    .await
                {
                    Ok(l) => {
                        if l != 2 * 2 * MAX_TAPS {
                            panic!("error sending all taps (bytes sent {})", l);
                        }
                    }
                    Err(e) => {
                        warn!("error sending taps ({:?})", e);
                    }
                }
                let mut send_buf = taps
                    .iter()
                    .flat_map(|v| v.to_be_bytes())
                    .collect::<Vec<u8>>();
                // prepend 'T' as message type to distinguish between [P]osition, [T]aps, and [M]ode
                send_buf.insert(0_usize, b'T');
                if let Err(e) = to_gui_udp_handler_tx_2.send(send_buf.clone()) {
                    warn!("error sending Filter Taps to gui ({:?})", e);
                }
                debug!("sent message to handler: {:?}", send_buf);
            }
        }
    }
}
