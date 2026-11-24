#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RenderQuality(u8);

impl RenderQuality {
    pub(crate) const FULL: Self = Self(0);
    const LOWEST: u8 = 3;

    pub(crate) fn target_size(self, native: [u32; 2], pixel_grid: bool) -> [u32; 2] {
        if pixel_grid || self == Self::FULL {
            return native;
        }
        let percent = [100, 85, 70, 55][self.0 as usize];
        native.map(|side| ((side * percent + 50) / 100).max(1))
    }

    pub(crate) fn shadow_scale(self) -> f32 {
        [1.0, 0.65, 0.0, 0.0][self.0 as usize]
    }
}

pub(crate) struct AdaptiveQuality {
    quality: RenderQuality,
    frame_average: Option<f32>,
    slow_for: f32,
    fast_for: f32,
    settle_for: f32,
}

impl Default for AdaptiveQuality {
    fn default() -> Self {
        Self {
            quality: RenderQuality::FULL,
            frame_average: None,
            slow_for: 0.0,
            fast_for: 0.0,
            settle_for: 0.5,
        }
    }
}

impl AdaptiveQuality {
    pub(crate) fn quality(&self) -> RenderQuality {
        self.quality
    }

    pub(crate) fn sample(&mut self, seconds: Option<f32>) {
        let Some(seconds) = seconds.filter(|dt| *dt > 0.0 && *dt < 1.0) else {
            self.clear_samples();
            return;
        };
        let interval = seconds.min(0.1);
        if self.settle_for > 0.0 {
            self.settle_for -= interval;
            return;
        }
        let average = self
            .frame_average
            .map_or(interval, |average| average + (interval - average) * 0.15);
        self.frame_average = Some(average);
        if average > 1.0 / 48.0 {
            self.slow_for += interval;
            self.fast_for = 0.0;
        } else if average < 1.0 / 54.0 {
            self.slow_for = 0.0;
            self.fast_for += interval;
        } else {
            self.slow_for = 0.0;
            self.fast_for = 0.0;
        }

        // Downgrade quickly and wait for sustained headroom before increasing load.
        if self.slow_for >= 0.5 && self.quality.0 < RenderQuality::LOWEST {
            self.quality.0 += 1;
            self.clear_samples();
        } else if self.fast_for >= 4.0 && self.quality != RenderQuality::FULL {
            self.quality.0 -= 1;
            self.clear_samples();
        }
    }

    fn clear_samples(&mut self) {
        self.frame_average = None;
        self.slow_for = 0.0;
        self.fast_for = 0.0;
        self.settle_for = 0.5;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(quality: &mut AdaptiveQuality, fps: f32, count: usize) {
        for _ in 0..count {
            quality.sample(Some(1.0 / fps));
        }
    }

    #[test]
    fn sustained_slow_frames_reduce_render_work_and_stable_frames_restore_it() {
        let mut quality = AdaptiveQuality::default();
        frames(&mut quality, 60.0, 90);
        frames(&mut quality, 30.0, 22);
        assert_eq!(
            quality.quality().target_size([1920, 1080], false),
            [1632, 918]
        );
        frames(&mut quality, 30.0, 120);
        assert_eq!(
            quality.quality().target_size([1920, 1080], false),
            [1056, 594]
        );
        assert_eq!(quality.quality().shadow_scale(), 0.0);
        assert_eq!(
            quality.quality().target_size([1919, 1079], true),
            [1919, 1079]
        );
        frames(&mut quality, 60.0, 180);
        assert_eq!(quality.quality(), RenderQuality(3));
        frames(&mut quality, 60.0, 900);
        assert_eq!(quality.quality(), RenderQuality::FULL);
    }

    #[test]
    fn a_single_hitch_background_time_and_near_target_jitter_keep_quality() {
        let mut quality = AdaptiveQuality::default();
        frames(&mut quality, 60.0, 120);
        quality.sample(Some(0.6));
        frames(&mut quality, 60.0, 60);
        quality.sample(None);
        quality.sample(Some(4.0));
        for fps in [50.0, 53.0].into_iter().cycle().take(240) {
            frames(&mut quality, fps, 1);
        }
        assert_eq!(quality.quality(), RenderQuality::FULL);
    }
}
