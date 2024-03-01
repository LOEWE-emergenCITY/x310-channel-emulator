// use log::debug;
// use log::info;
// use log::warn;
use pyo3::prelude::*;
use std::f32::consts::PI;
use std::sync::Mutex;
use num::complex::Complex64;
use crate::Polarization::Vertical;
use physical_constants;
use rand_distr::{Distribution, Normal, Uniform};
use rand::prelude::{thread_rng, ThreadRng};

/// debug script-local to enable/disabel certain debugging outputs with commenting out a single block of code
macro_rules! dsl {
    ($($tts:tt)*) => {
        // println!($($tts)*);
    }
}

#[derive(PartialEq)]
enum Polarization{
    Vertical,
    Horizontal
}

const STATION_X: f32 = 0.0;
const STATION_Y: f32 = 0.0;
pub const STATION_Z: f32 = 1.5;

const SPEED_OF_LIGHT: f32 = 299_792_458.;
pub const FREQUENCY: f32 = 2.45e9;
pub const LAMBDA: f32 = SPEED_OF_LIGHT / FREQUENCY;
const ANTENNA_SIZE: f32 = 0.1;  // metres
pub const FAR_FIELD_DISTANCE: f32 = (2. * ANTENNA_SIZE * ANTENNA_SIZE) / LAMBDA;
// const LAMBDA: f32 = 299_792_458. / 2.45e9;
// const EPSILON_R: f32 = 1.02;
const KA: f64 = 4.0/3.0 * 6_378_000.0;  // approx. earth radius in meters

const SMOOTING_FACTOR: f64 = 0.8;
const POLARIZATION: Polarization = Vertical;  // antennas mounted orthogonal to earths surface (e.g. straight downward from uav)

#[pyfunction]
pub fn get_station_z() -> f32 {
    STATION_Z
}

#[pyfunction]
pub fn distance(x: f32, y: f32, z: f32) -> f32 {
    ((STATION_X - x).powi(2) + (STATION_Y - y).powi(2) + (STATION_Z - z).powi(2)).sqrt()
}

pub fn dist_to_loss(dist: f32) -> f32 {
    4. * PI * (dist / LAMBDA)  // TODO
    // 1.0 + 4. * PI * (dist / LAMBDA)
}

#[pyfunction]
pub fn calculate_paths_freespace(x: f32, y: f32, z: f32) -> Vec<(f32, f32, f32)> {
    let dist = distance(x, y, z);
    let loss = dist_to_loss(dist);
    vec![(loss, 0.0_f32, 0.)]
}

#[pyfunction]
pub fn calculate_paths_two_ray(x: f32, y: f32, z: f32) -> Vec<(f32, f32, f32)> {
    let d_los = distance(x, y, z);
    let d_nlos = distance(x, y, z + 2. * STATION_Z);
    let delta_d = d_nlos - d_los;
    let delta_t = delta_d / SPEED_OF_LIGHT;
    let loss_los = dist_to_loss(d_los);
    let loss_nlos = dist_to_loss(d_nlos);
    vec![(loss_los, 0., 0.), (loss_nlos, delta_t, 0.)]
}

/// see DOI 10.1109/TVT.2016.2530306
#[pyfunction]
pub fn calculate_paths_ce2r(x: f32, y: f32, z: f32) -> Vec<(f32, f32, f32)> {
    dsl!("x,y,z {},{},{}", x, y, z);
    let r_1 = distance(x, y, z);
    let loss_los = dist_to_loss(r_1);
    let r_1 = r_1 as f64;
    dsl!("r_1 {}", r_1);
    // earth radius and absolute heights
    dsl!("KA {}", KA);
    let abs_height_uav = KA + z as f64;
    dsl!("abs_height_uav {}", abs_height_uav);
    let abs_height_station = KA + STATION_Z as f64;
    dsl!("abs_height_station {}", abs_height_station);
    let x_y_dist = distance(x, y, STATION_Z) as f64;
    // angle between uav and station via earth canter
    let q: f64 = if x_y_dist != 0.0 {
        (
            (abs_height_uav.powi(2) + abs_height_station.powi(2) - r_1.powi(2))
                /
                (2.0_f64 * abs_height_uav * abs_height_station)
        ).acos()  // angle theta_1 + theta_2
    }
    else {
        0.0_f64
    };
    // dsl!("r_1.powi(2) {}", (r_1 as f64).powi(2));
    // dsl!("q_no_acos {}", (
    //     (abs_height_uav.powi(2) + abs_height_station.powi(2) - (r_1 as f64).powi(2))
    //         /
    //     (2.0_f64*abs_height_uav*abs_height_station)
    // ));
    dsl!("q {}", q);
    // distance along earth surface between orthogonal projections of uav and station onto surface
    let d: f64 = KA * q;  // in meters
    dsl!("d {}", d);
    // intermediate quantities
    let m: f64 = d.powi(2) / (4.0 * KA * (z as f64 + STATION_Z as f64));
    let c: f64 = (z as f64 - STATION_Z as f64) / (z as f64 + STATION_Z as f64);
    let b: f64 = 2.0 * ((m + 1.0) / (3.0 * m)).sqrt() * ((PI / 3.0) as f64 + (3.0 * c * ((3.0 * m) / (m + 1.0).powi(3)).sqrt() / 2.0).acos() / 3.0).cos();
    let b = b.clamp(-1.0, 1.0);
    dsl!("m {}", m);
    dsl!("c {}", c);
    dsl!("b {}", b);
    // dsl!("((m + 1.0)/(3.0*m)).sqrt() {}", ((m + 1.0)/(3.0*m)).sqrt());
    // dsl!("3.0*c*((3.0*m)/(m+1.0).powi(3)).sqrt()/2.0) {}", 3.0*c*((3.0*m)/(m+1.0).powi(3)).sqrt()/2.0);
    // dsl!("(3.0*m)/(m+1.0).powi(3) {}", (3.0*m)/(m+1.0).powi(3));
    // dsl!("(PI/3.0 + (3.0*c*((3.0*m)/(m+1.0).powi(3)).sqrt()/2.0).acos()/3.0).cos() {}", ((PI/3.0) as f64+ (3.0*c*((3.0*m)/(m+1.0).powi(3)).sqrt()/2.0).acos()/3.0).cos());
    // reflection point
    let d_1 = d * (1.0 + b) / 2.0;
    let d_1 = d_1.clamp(0.0_f64, d);  // TODO
    let d_2 = d - d_1;
    let theta_1 = d_1 / KA;
    dsl!("d_1 {}", d_1);
    dsl!("d_2 {}", d_2);
    dsl!("theta_1 {}", theta_1);
    // grazing angle
    let psi = if x_y_dist > 0.0 {
        let psi_ce: f64 = (((z + STATION_Z) as f64) / d) * (1.0 - (m * (1.0 + b.powi(2))));
        let d_station_reflection = STATION_Z as f64 * x_y_dist / (z + STATION_Z) as f64;
        let psi_fe = (STATION_Z as f64 / d_station_reflection).atan();
        let fade_value = 0.5_f64 + 0.5_f64 * ((x_y_dist - 10.0_f64) / SMOOTING_FACTOR).tanh();
        psi_ce * fade_value + psi_fe * (1.0_f64 - fade_value)
    }
    else {
        (PI / 2.0) as f64
    };
    dsl!("psi {}", psi);
    // path length difference
    let delta_r =  if x_y_dist != 0.0 {
         (2.0 * d_1 * d_2 * psi.powi(2)) / d
    }
    else {
        (z.min(STATION_Z) + STATION_Z) as f64
    };
    dsl!("delta_r {}", delta_r);
    // r_2
    let r_2 = r_1 + delta_r;
    dsl!("r_2 {}", r_2);
    // reflected path amplitude purely by distance
    let alpha_s = 1.0_f64 / (dist_to_loss(r_2 as f32) as f64);
    dsl!("alpha_s {}", alpha_s);
    // lengths l_1 and l_2
    let l_1 = (abs_height_uav.powi(2) + KA.powi(2) - 2.0*KA*abs_height_uav*(theta_1.cos())).sqrt();
    // let l_2 = r_2 - l_1;  // TODO
    let l_2 = (abs_height_station.powi(2) + KA.powi(2) - 2.0*KA*abs_height_station*((q-theta_1).cos())).sqrt();
    // divergence factor D
    let divergence = 1.0 / (1.0 + ((2.0*l_1*l_2)/(KA*psi.sin()*(l_1+l_2)))).sqrt();
    dsl!("divergence {}", divergence);
    let v = (PI/2.0) as f64 - q;
    let p = ((z + STATION_Z) as f64)*q.sin() / v.sin();
    let phi: f64 = ((r_1.powi(2) + abs_height_uav.powi(2) - abs_height_station.powi(2)) / (2.0*r_1*abs_height_uav)).acos();
    let beta = (r_1 * phi.sin() / p).asin();
    // elevation angle
    let _theta_e = PI as f64 - phi - beta;
    // surface roughness
    const S_G: f32 = 0.1;  // 0.1m standard deviation of earth surface near reflection point for urban/suburban setting, see 10.1109/TVT.2017.2659651
    let c_r = (4.0 * PI * S_G) as f64 * psi.sin() / LAMBDA as f64;
    let r_f = (-c_r.powi(2) / 2.0).exp() as f64;
    dsl!("r_f {}", r_f);
    // surface reflection coefficient
    let omega: f64 = (2.0*PI*FREQUENCY) as f64;
    // ground reflective constants for average fround taken from ISBN 978-0-471-98857-1
    let sigma = 0.005;
    let epsilon_r = 15.0;
    let x_r = sigma / (omega * physical_constants::VACUUM_ELECTRIC_PERMITTIVITY);
    let epsilon_minus_j_x = Complex64::new(epsilon_r, -x_r);
    let tmp_1 = (epsilon_minus_j_x - psi.cos().powi(2)).sqrt();
    let tmp_2 = if POLARIZATION == Polarization::Horizontal {
        Complex64::from(psi.sin())
    }
    else {
        epsilon_minus_j_x * Complex64::from(psi.sin())
    };
    let rho: Complex64 = (tmp_2 - tmp_1) / (tmp_2 + tmp_1);
    let (gamma_f, additional_phase_shift) = rho.to_polar();
    dsl!("gamma_f {}", gamma_f);
    let amplitude_reflected_ray = alpha_s * gamma_f * divergence * r_f;
    let loss_nlos = 1.0 / amplitude_reflected_ray;
    let delta_t = delta_r / (SPEED_OF_LIGHT as f64);
    dsl!("delta_t {}", delta_t);
    let paths = vec![(loss_los, 0., 0.), (loss_nlos as f32, delta_t as f32, additional_phase_shift as f32)];
    // println!("{:?}", paths);
    paths
}

fn generate_sample(parameters: (f32, f32, f32), dist: f32, mut rng: &mut ThreadRng) -> f32 {
    let intercept = parameters.0;
    let slope = parameters.1;
    let std_dev = parameters.2;
    10.0_f32.powf(intercept + slope * (dist - 19_000.0).max(0.0) + Normal::new(0.0, std_dev).unwrap().sample(&mut rng))
}

#[derive(Clone, Copy)]
struct IntermittentRayParameters {
    x: f32,
    y: f32,
    z: f32,
    is_on: bool,
    duration: f32,
}
// parameters from 'NEAR-URBAN CLEVELAND' settings
// 'step 1'
static INTERMITTENT_RAY_ORIGINS: Mutex<[IntermittentRayParameters; 7]> = Mutex::new([IntermittentRayParameters {x: 0., y: 0., z: -1.0e10, is_on: false, duration: 0.}; 7]);
static DISTRIBUTION_ON_PROBABILITIES: [(f32, f32, f32); 7] = [
    (0.4480, -0.1457, 0.906256034),
    (-2.3302, -0.0630, 0.844452485),
    (-2.3578, -0.1367, 0.88391176),
    (-2.0716, -0.2233, 0.845517593),
    (-1.9377, -0.2502, 0.500699511),
    (-4.1835, 0.3570, 0.0),
    (-6.2697, 0.9563, 0.0),
];
// −
// -

// Ma[^0-9]*([0-9.-]*) ([0-9.-]*) ([0-9.-]*)[^\n]*
//

// Med[^0-9]*([0-9.-]*) ([0-9.-]*) ([0-9.-]*)[^\n]*
//

// M[^0-9]*([0-9.-]*) ([0-9.-]*) ([0-9.-]*)[^\n]*
// \t($1, $2, $3),\n
static DISTRIBUTION_DURATION: [(f32, f32, f32); 7] = [
	(0.5513, -0.0450, 0.5195190083144215),
	(0.2883, 0.0037, 0.4635730794599704),
	(0.1246, -0.0212, 0.5709640969448079),
	(0.0022, 0.0036, 0.6687301398920195),
	(0.5779, 0.1470, 0.3752332607858744),
	(2.1444, 0.7495, 0.0),
	(1.5143, 0.5968, 0.0),
];
static DISTRIBUTION_EXCESS_DELAY: [(f32, f32, f32); 7] = [
	(2.3210, -0.0047, 0.34481879299133333),
	(2.4248, 0.0029, 0.3590264614203248),
	(2.4914, 0.0186, 0.31432467291003424),
	(2.5198, 0.0253, 0.35482389998420344),
	(2.6964, 0.0168, 0.0888819441731559),
	(2.7381, 0.0281, 0.0),
	(2.9929, -0.0343, 0.0),
];


/// 'step 2'
/// see DOI 10.1109/TVT.2017.2659651
#[pyfunction]
pub fn calculate_paths_9ray_suburban(x: f32, y: f32, z: f32) -> Vec<(f32, f32, f32)> {
    // println!("FLAG!!!");
    let mut rng = thread_rng();
    let mut paths = calculate_paths_ce2r(x, y, z);
    // println!("paths: {:?}", paths);
    let dist = distance(x, y, z);
    // println!("dist: {:?}", dist);
    let mut intermittent_ray_states = INTERMITTENT_RAY_ORIGINS.lock().unwrap();
    // 'step 3'
    for k in 0..7 {
        if k == 0 || intermittent_ray_states[k-1].is_on {
            let current_ray_parameters: &mut IntermittentRayParameters = &mut intermittent_ray_states[k];
            // 'step 4'
            let dist_to_ray_origin = ((current_ray_parameters.x - x).powi(2) + (current_ray_parameters.y - y).powi(2) + (current_ray_parameters.z - z).powi(2)).sqrt();
            if dist_to_ray_origin > current_ray_parameters.duration {
                // 'step 5'
                let on_probability = generate_sample(DISTRIBUTION_ON_PROBABILITIES[k], dist, &mut rng);
                let on_sample = Uniform::new(0.0, 1.0).sample(&mut rng);
                current_ray_parameters.is_on = on_sample < on_probability;
                // 'step 6'
                current_ray_parameters.duration = generate_sample(DISTRIBUTION_DURATION[k], dist, &mut rng);
                if !current_ray_parameters.is_on {
                    break;
                }
            }
            else if !current_ray_parameters.is_on {
                break;
            }
            // 'step 7'
            // let additional_loss_db = (Normal::new(-30.3, 4.1).unwrap().sample(&mut rng) as f32).min(0.0);
            let additional_loss_db = (Normal::new(30.3, 4.1).unwrap().sample(&mut rng) as f32).max(0.0);
            // println!("additional_loss_db: {}", additional_loss_db);
            let additional_loss_linear = 10.0_f32.powf(additional_loss_db / 20.0);
            // println!("additional_loss_linear: {}", additional_loss_linear);
            let phase_shift = Uniform::new(0.0, 2.0 * PI).sample(&mut rng);
            // 'step 8'
            let excess_delay = generate_sample(DISTRIBUTION_EXCESS_DELAY[k], dist, &mut rng).max(0.0);  // nanoseconds
            let excess_delay = excess_delay / 1_000_000_000.0;
            // println!("excess_delay: {}", excess_delay);
            // let tap_index = (excess_delay * 200_000_000.0) as usize;
            // println!("tap_index: {}", tap_index);
            // println!("ray: {}", k+2);
            // println!("additional_loss_db: {}", additional_loss_db);
            // println!("additional_loss_linear: {}", additional_loss_linear);
            paths.push((paths[0].0 * additional_loss_linear, excess_delay, phase_shift));
        }
    }
    // println!("paths: {:?}", paths);
    paths
}

/// A Python module implemented in Rust. The name of this function must match
/// the `lib.name` setting in the `Cargo.toml`, else Python will not be able to
/// import the module.
#[pymodule]
fn channel_models(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(distance, m)?)?;
    m.add_function(wrap_pyfunction!(calculate_paths_freespace, m)?)?;
    m.add_function(wrap_pyfunction!(calculate_paths_two_ray, m)?)?;
    m.add_function(wrap_pyfunction!(calculate_paths_ce2r, m)?)?;
    m.add_function(wrap_pyfunction!(calculate_paths_9ray_suburban, m)?)?;
    m.add_function(wrap_pyfunction!(get_station_z, m)?)?;
    Ok(())
}
