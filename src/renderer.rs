use glam::DVec2;
use image::{ImageBuffer, Rgb};
use rand::{thread_rng, Rng};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const CHANNELS: usize = 3;
pub const DEFAULT_SAMPLES_PER_PIXEL: u64 = 200;
const ORBIT_SAMPLE_BOUNDS: ComplexBounds = ComplexBounds {
    real_min: -2.0,
    real_max: 2.0,
    imaginary_min: -2.0,
    imaginary_max: 2.0,
};

#[derive(Debug, Clone, Copy, PartialEq)]
struct ComplexBounds {
    real_min: f64,
    real_max: f64,
    imaginary_min: f64,
    imaginary_max: f64,
}

impl ComplexBounds {
    fn area(self) -> f64 {
        (self.real_max - self.real_min) * (self.imaginary_max - self.imaginary_min)
    }
}

fn orbit_sample_bounds() -> ComplexBounds {
    ORBIT_SAMPLE_BOUNDS
}

#[derive(Debug, Clone)]
pub struct RenderSettings {
    pub worker_count: usize,
    pub resolution: usize,
    pub samples_per_pixel: u64,
    pub brightness: f64,
    pub power: f64,
    pub iterations: [usize; CHANNELS],
    pub weights: [f64; CHANNELS],
    pub real_min: f64,
    pub real_max: f64,
    pub imaginary_min: f64,
    pub imaginary_max: f64,
    pub escape_radius: f64,
    pub preview_interval_ms: u64,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            worker_count: 20,
            resolution: 450,
            samples_per_pixel: DEFAULT_SAMPLES_PER_PIXEL,
            brightness: 1.0,
            power: 2.0,
            iterations: [5_000, 500, 50],
            weights: [15.0, 15.0, 10.0],
            real_min: -2.0,
            real_max: 2.0,
            imaginary_min: -2.0,
            imaginary_max: 2.0,
            escape_radius: 2.0,
            preview_interval_ms: 250,
        }
    }
}

impl RenderSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=256).contains(&self.worker_count) {
            return Err(String::from("Workers must be between 1 and 256."));
        }

        if !(8..=12_000).contains(&self.resolution) {
            return Err(String::from("Resolution must be between 8 and 12000."));
        }

        if self.samples_per_pixel == 0 {
            return Err(String::from("Samples per pixel must be at least 1."));
        }

        if !self.brightness.is_finite() || self.brightness <= 0.0 {
            return Err(String::from("Brightness must be a finite value above 0."));
        }

        if !self.power.is_finite() || !(1.0..=12.0).contains(&self.power) {
            return Err(String::from("Power k must be between 1 and 12."));
        }

        if self.iterations.iter().any(|&value| value == 0) {
            return Err(String::from("Channel iterations must be at least 1."));
        }

        if self.max_iterations() > 1_000_000 {
            return Err(String::from(
                "Channel iterations are capped at 1000000 to keep worker memory bounded.",
            ));
        }

        if self
            .weights
            .iter()
            .any(|&value| !value.is_finite() || value <= 0.0)
        {
            return Err(String::from(
                "Channel weights must be finite values above 0.",
            ));
        }

        if !self.real_min.is_finite()
            || !self.real_max.is_finite()
            || !self.imaginary_min.is_finite()
            || !self.imaginary_max.is_finite()
            || self.real_min >= self.real_max
            || self.imaginary_min >= self.imaginary_max
        {
            return Err(String::from(
                "Complex plane bounds must be finite min/max pairs.",
            ));
        }

        if !self.escape_radius.is_finite() || self.escape_radius <= 0.0 {
            return Err(String::from(
                "Escape radius must be a finite value above 0.",
            ));
        }

        if !(16..=5_000).contains(&self.preview_interval_ms) {
            return Err(String::from(
                "Preview interval must be between 16 and 5000 milliseconds.",
            ));
        }

        let pixels = self
            .resolution
            .checked_mul(self.resolution)
            .ok_or_else(|| String::from("Resolution is too large."))?;
        pixels
            .checked_mul(CHANNELS)
            .and_then(|value| value.checked_mul(std::mem::size_of::<AtomicU64>()))
            .ok_or_else(|| String::from("Resolution requires too much memory."))?;

        self.total_samples_checked()
            .map(|_| ())
            .ok_or_else(|| String::from("Total sample count is too large."))
    }

    pub fn total_samples(&self) -> u64 {
        self.total_samples_checked()
            .expect("settings should be validated before rendering")
    }

    fn total_samples_checked(&self) -> Option<u64> {
        (self.worker_count as u64)
            .checked_mul(self.samples_per_pixel)?
            .checked_mul(self.resolution as u64)?
            .checked_mul(self.resolution as u64)
    }

    fn max_iterations(&self) -> usize {
        self.iterations.into_iter().max().unwrap_or(0)
    }

    fn channel_value(&self, channel: usize, hits: u64, completed_samples: u64) -> f64 {
        if completed_samples == 0 {
            return 0.0;
        }

        let virtual_resolution_area = self.virtual_resolution_area();
        hits as f64 * virtual_resolution_area * self.brightness
            / (self.weights[channel] * completed_samples as f64)
    }

    fn crop_bounds(&self) -> ComplexBounds {
        ComplexBounds {
            real_min: self.real_min,
            real_max: self.real_max,
            imaginary_min: self.imaginary_min,
            imaginary_max: self.imaginary_max,
        }
    }

    fn virtual_resolution_area(&self) -> f64 {
        let output_pixels = (self.resolution * self.resolution) as f64;
        output_pixels * orbit_sample_bounds().area() / self.crop_bounds().area()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ToneMapping {
    pub gamma: f64,
}

impl ToneMapping {
    pub fn new(gamma: f64) -> Self {
        Self {
            gamma: gamma.clamp(0.2, 4.0),
        }
    }

    fn apply(self, value: f64) -> f64 {
        value.max(0.0).powf(1.0 / self.gamma)
    }
}

pub struct RenderSnapshot {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub completed_samples: u64,
    pub total_samples: u64,
}

pub struct RenderJob {
    buffers: Arc<RenderBuffers>,
    thread: Option<JoinHandle<()>>,
}

impl RenderJob {
    pub fn start(settings: RenderSettings) -> Self {
        let buffers = Arc::new(RenderBuffers::new(settings));
        let thread_buffers = Arc::clone(&buffers);
        let thread = thread::spawn(move || render(thread_buffers));

        Self {
            buffers,
            thread: Some(thread),
        }
    }

    pub fn cancel(&self) {
        self.buffers.cancelled.store(true, Ordering::Relaxed);
        self.buffers.paused.store(false, Ordering::Relaxed);
    }

    pub fn pause(&self) {
        if !self.is_finished() {
            self.buffers.paused.store(true, Ordering::Relaxed);
        }
    }

    pub fn resume(&self) {
        self.buffers.paused.store(false, Ordering::Relaxed);
    }

    pub fn is_paused(&self) -> bool {
        self.buffers.paused.load(Ordering::Relaxed)
    }

    pub fn was_cancelled(&self) -> bool {
        self.buffers.cancelled.load(Ordering::Relaxed)
    }

    pub fn is_finished(&self) -> bool {
        self.buffers.finished.load(Ordering::Relaxed)
    }

    pub fn join_if_finished(&mut self) {
        let Some(thread) = &self.thread else {
            return;
        };

        if thread.is_finished() {
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    pub fn snapshot(&self, tone_mapping: ToneMapping) -> RenderSnapshot {
        self.buffers.snapshot(tone_mapping)
    }

    pub fn save_png<P: AsRef<Path>>(
        &self,
        path: P,
        tone_mapping: ToneMapping,
    ) -> image::ImageResult<()> {
        self.buffers.save_png(path, tone_mapping)
    }

    pub fn extend_samples(&mut self) -> Result<u64, String> {
        let new_total = self.buffers.extend_by_base_budget()?;

        self.buffers.cancelled.store(false, Ordering::Relaxed);
        self.buffers.paused.store(false, Ordering::Relaxed);
        self.buffers.finished.store(false, Ordering::Relaxed);

        if self
            .thread
            .as_ref()
            .map(|thread| thread.is_finished())
            .unwrap_or(true)
        {
            self.join_if_finished();
            let thread_buffers = Arc::clone(&self.buffers);
            self.thread = Some(thread::spawn(move || render(thread_buffers)));
        }

        Ok(new_total)
    }
}

impl Drop for RenderJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

struct RenderBuffers {
    settings: RenderSettings,
    channels: [Vec<AtomicU64>; CHANNELS],
    completed_samples: AtomicU64,
    target_samples: AtomicU64,
    cancelled: AtomicBool,
    paused: AtomicBool,
    finished: AtomicBool,
}

impl RenderBuffers {
    fn new(settings: RenderSettings) -> Self {
        let pixel_count = settings.resolution * settings.resolution;
        let target_samples = settings.total_samples();
        let channels = std::array::from_fn(|_| {
            (0..pixel_count)
                .map(|_| AtomicU64::new(0))
                .collect::<Vec<_>>()
        });

        Self {
            settings,
            channels,
            completed_samples: AtomicU64::new(0),
            target_samples: AtomicU64::new(target_samples),
            cancelled: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            finished: AtomicBool::new(false),
        }
    }

    fn extend_by_base_budget(&self) -> Result<u64, String> {
        let additional_samples = self.settings.total_samples();
        let mut current = self.target_samples.load(Ordering::Relaxed);

        loop {
            let next = current
                .checked_add(additional_samples)
                .ok_or_else(|| String::from("Extended sample count is too large."))?;

            match self.target_samples.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(next),
                Err(value) => current = value,
            }
        }
    }

    fn target_sample_count(&self) -> u64 {
        self.target_samples.load(Ordering::Relaxed)
    }

    fn snapshot(&self, tone_mapping: ToneMapping) -> RenderSnapshot {
        let resolution = self.settings.resolution;
        let mut rgba = Vec::with_capacity(resolution * resolution * 4);
        let completed_samples = self.completed_sample_count();

        for y in 0..resolution {
            for x in 0..resolution {
                let index = self.pixel_index(x, y);

                for channel in 0..CHANNELS {
                    let hits = self.channels[channel][index].load(Ordering::Relaxed);
                    let linear = self
                        .settings
                        .channel_value(channel, hits, completed_samples);
                    let value = tone_mapping.apply(linear);
                    rgba.push((value * 255.0).clamp(0.0, 255.0) as u8);
                }

                rgba.push(255);
            }
        }

        RenderSnapshot {
            width: resolution as u32,
            height: resolution as u32,
            rgba,
            completed_samples,
            total_samples: self.target_sample_count(),
        }
    }

    fn save_png<P: AsRef<Path>>(
        &self,
        path: P,
        tone_mapping: ToneMapping,
    ) -> image::ImageResult<()> {
        let resolution = self.settings.resolution;
        let completed_samples = self.completed_sample_count();
        let mut image: ImageBuffer<Rgb<u16>, Vec<u16>> =
            ImageBuffer::new(resolution as u32, resolution as u32);

        for y in 0..resolution {
            for x in 0..resolution {
                let index = self.pixel_index(x, y);
                let pixel = [
                    self.channel_u16(0, index, completed_samples, tone_mapping),
                    self.channel_u16(1, index, completed_samples, tone_mapping),
                    self.channel_u16(2, index, completed_samples, tone_mapping),
                ];

                *image.get_pixel_mut(x as u32, y as u32) = Rgb(pixel);
            }
        }

        image.save(path)
    }

    fn completed_sample_count(&self) -> u64 {
        self.completed_samples
            .load(Ordering::Relaxed)
            .min(self.target_sample_count())
    }

    fn channel_u16(
        &self,
        channel: usize,
        index: usize,
        completed_samples: u64,
        tone_mapping: ToneMapping,
    ) -> u16 {
        let hits = self.channels[channel][index].load(Ordering::Relaxed);
        let linear = self
            .settings
            .channel_value(channel, hits, completed_samples);
        let value = tone_mapping.apply(linear);
        (value * u16::MAX as f64).clamp(0.0, u16::MAX as f64) as u16
    }

    fn pixel_index(&self, x: usize, y: usize) -> usize {
        x * self.settings.resolution + y
    }
}

fn render(buffers: Arc<RenderBuffers>) {
    let settings = buffers.settings.clone();
    let max_iterations = settings.max_iterations();
    let escape_radius_squared = settings.escape_radius * settings.escape_radius;
    let power = ComplexPower::from(settings.power);

    thread::scope(|scope| {
        for _ in 0..settings.worker_count {
            let buffers = Arc::clone(&buffers);
            let settings = settings.clone();

            scope.spawn(move || {
                let mut rng = thread_rng();
                let mut orbit = vec![Cmplx::ZERO; max_iterations + 1];

                loop {
                    if buffers.cancelled.load(Ordering::Relaxed) {
                        break;
                    }

                    while buffers.paused.load(Ordering::Relaxed)
                        && !buffers.cancelled.load(Ordering::Relaxed)
                    {
                        thread::sleep(Duration::from_millis(20));
                    }

                    if buffers.cancelled.load(Ordering::Relaxed)
                        || !claim_sample(&buffers.completed_samples, &buffers.target_samples)
                    {
                        break;
                    }

                    let sample_bounds = orbit_sample_bounds();
                    let c = Cmplx::new(
                        rng.gen_range(sample_bounds.real_min..sample_bounds.real_max),
                        rng.gen_range(sample_bounds.imaginary_min..sample_bounds.imaginary_max),
                    );
                    let mut z = Cmplx::ZERO;

                    let mut steps = 0;

                    while steps <= max_iterations && z.length_squared() < escape_radius_squared {
                        z = z.pow(power) + c;
                        orbit[steps] = z;
                        steps += 1;
                    }

                    for channel in 0..CHANNELS {
                        let channel_iterations = settings.iterations[channel];

                        if steps == 0 || steps > channel_iterations {
                            continue;
                        }

                        record_orbit_hits(
                            &buffers.channels[channel],
                            &settings,
                            &orbit[..channel_iterations.min(steps)],
                        );
                    }
                }
            });
        }
    });

    buffers.finished.store(true, Ordering::Relaxed);
}

fn claim_sample(counter: &AtomicU64, target_samples: &AtomicU64) -> bool {
    let mut current = counter.load(Ordering::Relaxed);

    loop {
        let total_samples = target_samples.load(Ordering::Relaxed);
        if current >= total_samples {
            return false;
        }

        match counter.compare_exchange_weak(
            current,
            current + 1,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return true,
            Err(next) => current = next,
        }
    }
}

fn record_orbit_hits(channel: &[AtomicU64], settings: &RenderSettings, points: &[Cmplx]) {
    for point in points {
        if let Some(index) = complex_to_pixel_index(*point, settings) {
            channel[index].fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn complex_to_pixel_index(value: Cmplx, settings: &RenderSettings) -> Option<usize> {
    let x = (value.data.x - settings.real_min) / (settings.real_max - settings.real_min);
    let y =
        (value.data.y - settings.imaginary_min) / (settings.imaginary_max - settings.imaginary_min);

    if !(0.0..1.0).contains(&x) || !(0.0..1.0).contains(&y) {
        return None;
    }

    let pixel_x = (x * settings.resolution as f64) as usize;
    let pixel_y = (y * settings.resolution as f64) as usize;

    Some(pixel_x * settings.resolution + pixel_y)
}

#[derive(Default, Debug, PartialEq, Copy, Clone)]
struct Cmplx {
    data: DVec2,
}

impl Cmplx {
    const ZERO: Self = Self::new(0.0, 0.0);

    const fn new(x: f64, y: f64) -> Self {
        Self {
            data: DVec2 { x, y },
        }
    }

    fn length_squared(&self) -> f64 {
        self.data.length_squared()
    }

    fn pow(self, power: ComplexPower) -> Self {
        match power {
            ComplexPower::One => self,
            ComplexPower::Two => self * self,
            ComplexPower::Integer(exponent) => self.pow_integer(exponent),
            ComplexPower::Real(power) => self.pow_real(power),
        }
    }

    fn pow_integer(self, exponent: u32) -> Self {
        let mut result = Self::new(1.0, 0.0);
        let mut base = self;
        let mut exponent = exponent;

        while exponent > 0 {
            if exponent % 2 == 1 {
                result = result * base;
            }

            base = base * base;
            exponent /= 2;
        }

        result
    }

    fn pow_real(self, power: f64) -> Self {
        if self.data.x == 0.0 && self.data.y == 0.0 {
            return Self::ZERO;
        }

        let radius = self.data.length().powf(power);
        let angle = self.data.y.atan2(self.data.x) * power;

        Self::new(radius * angle.cos(), radius * angle.sin())
    }
}

#[derive(Debug, Clone, Copy)]
enum ComplexPower {
    One,
    Two,
    Integer(u32),
    Real(f64),
}

impl From<f64> for ComplexPower {
    fn from(value: f64) -> Self {
        if (value - 1.0).abs() < f64::EPSILON {
            Self::One
        } else if (value - 2.0).abs() < f64::EPSILON {
            Self::Two
        } else {
            let rounded = value.round();
            if (value - rounded).abs() < f64::EPSILON && rounded >= 1.0 {
                Self::Integer(rounded as u32)
            } else {
                Self::Real(value)
            }
        }
    }
}

impl std::ops::Mul for Cmplx {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(
            self.data.x * rhs.data.x - self.data.y * rhs.data.y,
            self.data.x * rhs.data.y + self.data.y * rhs.data.x,
        )
    }
}

impl std::ops::Add for Cmplx {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        (self.data + rhs.data).into()
    }
}

impl From<DVec2> for Cmplx {
    fn from(value: DVec2) -> Self {
        Self { data: value }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn small_render_finishes() {
        let settings = RenderSettings {
            worker_count: 1,
            resolution: 8,
            samples_per_pixel: 1,
            brightness: 1.0,
            power: 2.0,
            iterations: [8, 4, 2],
            weights: [1.0, 1.0, 1.0],
            real_min: -2.0,
            real_max: 2.0,
            imaginary_min: -2.0,
            imaginary_max: 2.0,
            escape_radius: 2.0,
            preview_interval_ms: 50,
        };

        settings.validate().unwrap();

        let mut job = RenderJob::start(settings);

        while !job.is_finished() {
            std::thread::sleep(Duration::from_millis(5));
        }

        job.join_if_finished();
        let snapshot = job.snapshot(ToneMapping::new(1.0));

        assert_eq!(snapshot.width, 8);
        assert_eq!(snapshot.height, 8);
        assert_eq!(snapshot.completed_samples, snapshot.total_samples);
        assert_eq!(snapshot.rgba.len(), 8 * 8 * 4);
    }

    #[test]
    fn completed_render_can_extend_for_more_samples() {
        let settings = RenderSettings {
            worker_count: 1,
            resolution: 8,
            samples_per_pixel: 1,
            brightness: 1.0,
            power: 2.0,
            iterations: [8, 4, 2],
            weights: [1.0, 1.0, 1.0],
            real_min: -2.0,
            real_max: 2.0,
            imaginary_min: -2.0,
            imaginary_max: 2.0,
            escape_radius: 2.0,
            preview_interval_ms: 50,
        };

        let mut job = RenderJob::start(settings);

        while !job.is_finished() {
            std::thread::sleep(Duration::from_millis(5));
        }

        job.join_if_finished();
        let first_snapshot = job.snapshot(ToneMapping::new(1.0));
        let extended_total = job.extend_samples().unwrap();

        assert_eq!(extended_total, first_snapshot.total_samples * 2);
        assert_eq!(
            job.snapshot(ToneMapping::new(1.0)).completed_samples,
            first_snapshot.completed_samples
        );

        while !job.is_finished() {
            std::thread::sleep(Duration::from_millis(5));
        }

        job.join_if_finished();
        let final_snapshot = job.snapshot(ToneMapping::new(1.0));

        assert_eq!(final_snapshot.total_samples, extended_total);
        assert_eq!(final_snapshot.completed_samples, extended_total);
    }

    #[test]
    fn paused_render_can_resume() {
        let settings = RenderSettings {
            worker_count: 1,
            resolution: 16,
            samples_per_pixel: 2,
            brightness: 1.0,
            power: 2.0,
            iterations: [16, 8, 4],
            weights: [1.0, 1.0, 1.0],
            real_min: -2.0,
            real_max: 2.0,
            imaginary_min: -2.0,
            imaginary_max: 2.0,
            escape_radius: 2.0,
            preview_interval_ms: 50,
        };

        settings.validate().unwrap();

        let mut job = RenderJob::start(settings);
        job.pause();
        assert!(job.is_paused());
        job.resume();

        while !job.is_finished() {
            std::thread::sleep(Duration::from_millis(5));
        }

        job.join_if_finished();

        assert!(!job.is_paused());
        assert_eq!(
            job.snapshot(ToneMapping::new(1.0)).completed_samples,
            job.snapshot(ToneMapping::new(1.0)).total_samples
        );
    }

    #[test]
    fn higher_power_render_finishes() {
        let settings = RenderSettings {
            worker_count: 1,
            resolution: 8,
            samples_per_pixel: 1,
            brightness: 1.0,
            power: 3.0,
            iterations: [8, 4, 2],
            weights: [1.0, 1.0, 1.0],
            real_min: -2.0,
            real_max: 2.0,
            imaginary_min: -2.0,
            imaginary_max: 2.0,
            escape_radius: 2.0,
            preview_interval_ms: 50,
        };

        settings.validate().unwrap();

        let mut job = RenderJob::start(settings);

        while !job.is_finished() {
            std::thread::sleep(Duration::from_millis(5));
        }

        job.join_if_finished();

        assert_eq!(
            job.snapshot(ToneMapping::new(1.0)).completed_samples,
            job.snapshot(ToneMapping::new(1.0)).total_samples
        );
    }

    #[test]
    fn brightness_scales_raw_hits_by_completed_samples() {
        let settings = RenderSettings {
            worker_count: 1,
            resolution: 10,
            samples_per_pixel: 1,
            brightness: 2.0,
            power: 2.0,
            iterations: [8, 4, 2],
            weights: [5.0, 1.0, 1.0],
            real_min: -2.0,
            real_max: 2.0,
            imaginary_min: -2.0,
            imaginary_max: 2.0,
            escape_radius: 2.0,
            preview_interval_ms: 50,
        };

        assert_eq!(settings.channel_value(0, 3, 200), 0.6);
        assert_eq!(settings.channel_value(0, 3, 0), 0.0);
    }

    #[test]
    fn brightness_scales_crop_as_virtual_full_resolution_area() {
        let settings = RenderSettings {
            worker_count: 1,
            resolution: 10,
            samples_per_pixel: 1,
            brightness: 2.0,
            power: 2.0,
            iterations: [8, 4, 2],
            weights: [5.0, 1.0, 1.0],
            real_min: -1.0,
            real_max: 1.0,
            imaginary_min: -1.0,
            imaginary_max: 1.0,
            escape_radius: 2.0,
            preview_interval_ms: 50,
        };

        assert_eq!(settings.virtual_resolution_area(), 400.0);
        assert_eq!(settings.channel_value(0, 3, 200), 2.4);
    }

    #[test]
    fn gamma_is_applied_at_readout_time() {
        let tone_mapping = ToneMapping::new(2.0);

        assert_eq!(tone_mapping.apply(0.25), 0.5);
    }

    #[test]
    fn crop_bounds_do_not_change_orbit_sampling_bounds() {
        let mut settings = RenderSettings::default();
        settings.real_min = -0.25;
        settings.real_max = 0.25;
        settings.imaginary_min = -0.5;
        settings.imaginary_max = 0.5;

        assert_eq!(
            orbit_sample_bounds(),
            ComplexBounds {
                real_min: -2.0,
                real_max: 2.0,
                imaginary_min: -2.0,
                imaginary_max: 2.0,
            }
        );
        settings.validate().unwrap();
    }

    #[test]
    fn orbit_points_outside_crop_are_skipped_without_stopping_orbit() {
        let settings = RenderSettings {
            worker_count: 1,
            resolution: 8,
            samples_per_pixel: 1,
            brightness: 1.0,
            power: 2.0,
            iterations: [8, 4, 2],
            weights: [1.0, 1.0, 1.0],
            real_min: -1.0,
            real_max: 1.0,
            imaginary_min: -1.0,
            imaginary_max: 1.0,
            escape_radius: 2.0,
            preview_interval_ms: 50,
        };
        let channel = (0..settings.resolution * settings.resolution)
            .map(|_| AtomicU64::new(0))
            .collect::<Vec<_>>();
        let points = [
            Cmplx::new(-1.5, 0.0),
            Cmplx::new(0.0, 0.0),
            Cmplx::new(1.5, 0.0),
        ];

        record_orbit_hits(&channel, &settings, &points);

        let hits = channel
            .iter()
            .map(|value| value.load(Ordering::Relaxed))
            .sum::<u64>();

        assert_eq!(hits, 1);
    }
}
