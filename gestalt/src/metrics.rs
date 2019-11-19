use std::time::Instant;
use std::collections::VecDeque;
use crate::pipeline::text::TextData;


const AVG_SAMPLE_COUNT: usize = 20;


pub struct FrameMetrics {
    // timestamps of completed frames, for fps. entries > 1 sec ago are removed
    completed_frames: Vec<Instant>,
    // timestamps for calculating individual segment times
    pub frame_start_time: Instant,
    game_end_time: Instant,
    draw_end_time: Instant,
    gpu_end_time: Instant,
    // text data from previous frame
    last_frame_text_data: Vec<TextData>,
    // timestamp to rate-limit display
    last_text_update: Instant,
    // segment time samples for averaging
    game_time_samples: VecDeque<f32>,
    draw_time_samples: VecDeque<f32>,
    gpu_time_samples: VecDeque<f32>,
}

impl FrameMetrics {
    pub fn new() -> Self {
        Self {
            completed_frames: Vec::new(),
            frame_start_time: Instant::now(),
            game_end_time: Instant::now(),
            draw_end_time: Instant::now(),
            gpu_end_time: Instant::now(),
            last_frame_text_data: Vec::new(),
            last_text_update: Instant::now(),
            game_time_samples: VecDeque::new(),
            draw_time_samples: VecDeque::new(),
            gpu_time_samples: VecDeque::new(),
        }
    }

    pub fn get_text(&mut self, pos: (i32, i32)) -> Vec<TextData> {
        self.last_frame_text_data.iter().enumerate().clone()
            .map(|(idx, text)| TextData {
                position: (pos.0, pos.1 + (15 * idx as i32)),
                ..text.clone()
            })
            .collect()
    }

    pub fn start_frame(&mut self) -> std::time::Duration {
        let dt = Instant::now() - self.frame_start_time;
        self.frame_start_time = Instant::now();
        dt
    }

    pub fn end_game(&mut self) {
        self.game_end_time = Instant::now();
    }

    pub fn end_draw(&mut self) {
        self.draw_end_time = Instant::now();
    }

    pub fn end_gpu(&mut self) {
        self.gpu_end_time = Instant::now();
    }

    pub fn end_frame(&mut self) {
        let now = Instant::now();
        self.completed_frames = self.completed_frames
            .iter()
            .cloned()
            .filter(|inst| now.duration_since(*inst).as_secs_f32() < 1.0)
            .collect();
        self.completed_frames.push(now);

        // limit text update speed for readability
        if now.duration_since(self.last_text_update).subsec_millis() < 50 {
            return;
        }
        self.last_text_update = now;

        let game_time = self.game_end_time.duration_since(self.frame_start_time).subsec_micros() as f32 / 1000.0;
        let draw_time = self.draw_end_time.duration_since(self.frame_start_time).subsec_micros() as f32 / 1000.0;
        let  gpu_time =  self.gpu_end_time.duration_since(self.frame_start_time).subsec_micros() as f32 / 1000.0;

        self.game_time_samples.push_back(game_time);
        if self.game_time_samples.len() > AVG_SAMPLE_COUNT {
            self.game_time_samples.pop_front();
        }
        let game_time_avg = self.game_time_samples.iter().fold(0.0, |acc, x| acc + *x) / self.game_time_samples.len() as f32;
        self.draw_time_samples.push_back(draw_time);
        if self.draw_time_samples.len() > AVG_SAMPLE_COUNT {
            self.draw_time_samples.pop_front();
        }
        let draw_time_avg = self.draw_time_samples.iter().fold(0.0, |acc, x| acc + *x) / self.draw_time_samples.len() as f32;
        self.gpu_time_samples.push_back(gpu_time);
        if self.gpu_time_samples.len() > AVG_SAMPLE_COUNT {
            self.gpu_time_samples.pop_front();
        }
        let gpu_time_avg = self.gpu_time_samples.iter().fold(0.0, |acc, x| acc + *x) / self.gpu_time_samples.len() as f32;

        let fps = self.completed_frames.len();
        self.last_frame_text_data.clear();
        self.last_frame_text_data.push(TextData {
            text: format!("{} FPS", fps),
            position: (5, 5),
            ..TextData::default()
        });
        self.last_frame_text_data.push(TextData {
            text: format!("game:{:>5.1}ms /{:>5.1}ms", game_time, game_time_avg),
            position: (5, 20),
            family: "Fira Mono".into(),
            size: 16.0,
            ..TextData::default()
        });
        self.last_frame_text_data.push(TextData {
            text: format!("draw:{:>5.1}ms /{:>5.1}ms", draw_time, draw_time_avg),
            position: (5, 35),
            family: "Fira Mono".into(),
            size: 16.0,
            ..TextData::default()
        });
        self.last_frame_text_data.push(TextData {
            text: format!(" gpu:{:>5.1}ms /{:>5.1}ms", gpu_time, gpu_time_avg),
            position: (5, 50),
            family: "Fira Mono".into(),
            size: 16.0,
            ..TextData::default()
        });
    }
}