use num_rational::Ratio;

type ChannelCount = u32;
type SampleRate = u32;
type Sample = f32;



#[derive(Clone, Debug)]
pub struct SampleRateConverter<I> where I: Iterator {
    /// The iterator that gives us samples.
    pub input: I,
    /// We convert chunks of `from` samples into chunks of `to` samples.
    from: u32,
    /// We convert chunks of `from` samples into chunks of `to` samples.
    to: u32,
    /// Number of channels in the stream
    channels: ChannelCount,
    /// One sample per channel, extracted from `input`.
    current_span: Vec<I::Item>,
    /// Position of `current_sample` modulo `from`.
    current_span_pos_in_chunk: u32,
    /// The samples right after `current_sample` (one per channel), extracted from `input`.
    next_frame: Vec<I::Item>,
    /// The position of the next sample that the iterator should return, modulo `to`.
    /// This counter is incremented (modulo `to`) every time the iterator is called.
    next_output_span_pos_in_chunk: u32,
    /// The buffer containing the samples waiting to be output.
    output_buffer: Vec<I::Item>,
}

impl<I> SampleRateConverter<I>
where
    I: Iterator,
{
    /// Create new sample rate converter.
    ///
    /// The converter uses simple linear interpolation for up-sampling
    /// and discards samples for down-sampling. This may introduce audible
    /// distortions in some cases (see [#584](https://github.com/RustAudio/rodio/issues/584)).
    ///
    /// # Limitations
    /// Some rate conversions where target rate is high and rates are mutual primes the sample
    /// interpolation may cause numeric overflows. Conversion between usual sample rates
    /// 2400, 8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, ... is expected to work.
    ///
    /// # Panic
    /// Panics if `from`, `to` or `num_channels` are 0.
    #[inline]
    pub fn new(
        mut input: I,
        from: SampleRate,
        to: SampleRate,
        num_channels: ChannelCount,
    ) -> SampleRateConverter<I> {
        let (first_samples, next_samples) = if from == to {
            // if `from` == `to` == 1, then we just pass through
            (Vec::new(), Vec::new())
        } else {
            let first = input
                .by_ref()
                .take(num_channels as usize)
                .collect::<Vec<_>>();
            let next = input
                .by_ref()
                .take(num_channels as usize)
                .collect::<Vec<_>>();
            (first, next)
        };

        // Reducing numerator to avoid numeric overflows during interpolation.
        let (to, from) = Ratio::new(to, from).into_raw();

        SampleRateConverter {
            input,
            from,
            to,
            channels: num_channels,
            current_span_pos_in_chunk: 0,
            next_output_span_pos_in_chunk: 0,
            current_span: first_samples,
            next_frame: next_samples,
            output_buffer: Vec::with_capacity(num_channels as usize - 1),
        }
    }

    fn next_input_span(&mut self) {
        self.current_span_pos_in_chunk += 1;

        std::mem::swap(&mut self.current_span, &mut self.next_frame);
        self.next_frame.clear();
        for _ in 0..self.channels {
            if let Some(i) = self.input.next() {
                self.next_frame.push(i);
            } else {
                break;
            }
        }
    }
}

impl<I> Iterator for SampleRateConverter<I>
where
    I: Iterator<Item = Sample>,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<I::Item> {
        // the algorithm below doesn't work if `self.from == self.to`
        if self.from == self.to {
            debug_assert_eq!(self.from, 1);
            return self.input.next();
        }

        // Short circuit if there are some samples waiting.
        if !self.output_buffer.is_empty() {
            return Some(self.output_buffer.remove(0));
        }

        // The span we are going to return from this function will be a linear interpolation
        // between `self.current_span` and `self.next_span`.

        if self.next_output_span_pos_in_chunk == self.to {
            // If we jump to the next span, we reset the whole state.
            self.next_output_span_pos_in_chunk = 0;

            self.next_input_span();
            while self.current_span_pos_in_chunk != self.from {
                self.next_input_span();
            }
            self.current_span_pos_in_chunk = 0;
        } else {
            // Finding the position of the first sample of the linear interpolation.
            let req_left_sample =
                (self.from * self.next_output_span_pos_in_chunk / self.to) % self.from;

            // Advancing `self.current_span`, `self.next_span` and
            // `self.current_span_pos_in_chunk` until the latter variable
            // matches `req_left_sample`.
            while self.current_span_pos_in_chunk != req_left_sample {
                self.next_input_span();
                debug_assert!(self.current_span_pos_in_chunk < self.from);
            }
        }

        // Merging `self.current_span` and `self.next_span` into `self.output_buffer`.
        // Note that `self.output_buffer` can be truncated if there is not enough data in
        // `self.next_span`.
        let mut result = None;
        let numerator = (self.from * self.next_output_span_pos_in_chunk) % self.to;
        for (off, (cur, next)) in self
            .current_span
            .iter()
            .zip(self.next_frame.iter())
            .enumerate()
        {
            let sample = lerp(cur, next, numerator, self.to);

            if off == 0 {
                result = Some(sample);
            } else {
                self.output_buffer.push(sample);
            }
        }

        // Incrementing the counter for the next iteration.
        self.next_output_span_pos_in_chunk += 1;

        if result.is_some() {
            result
        } else {
            // draining `self.current_span`
            if !self.current_span.is_empty() {
                let r = Some(self.current_span.remove(0));
                std::mem::swap(&mut self.output_buffer, &mut self.current_span);
                self.current_span.clear();
                r
            } else {
                None
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let apply = |samples: usize| {
            // `samples_after_chunk` will contain the number of samples remaining after the chunk
            // currently being processed
            let samples_after_chunk = samples;
            // adding the samples of the next chunk that may have already been read
            let samples_after_chunk = if self.current_span_pos_in_chunk == self.from - 1 {
                samples_after_chunk + self.next_frame.len()
            } else {
                samples_after_chunk
            };
            // removing the samples of the current chunk that have not yet been read
            let samples_after_chunk = samples_after_chunk.saturating_sub(
                self.from.saturating_sub(self.current_span_pos_in_chunk + 2) as usize
                    * usize::from(self.channels as usize),
            );
            // calculating the number of samples after the transformation
            // TODO: this is wrong here \|/
            let samples_after_chunk = samples_after_chunk * self.to as usize / self.from as usize;

            // `samples_current_chunk` will contain the number of samples remaining to be output
            // for the chunk currently being processed
            let samples_current_chunk = (self.to - self.next_output_span_pos_in_chunk) as usize
                * usize::from(self.channels as usize);

            samples_current_chunk + samples_after_chunk + self.output_buffer.len()
        };

        if self.from == self.to {
            self.input.size_hint()
        } else {
            let (min, max) = self.input.size_hint();
            (apply(min), max.map(apply))
        }
    }
}

#[inline]
pub(crate) fn lerp(first: &f32, second: &f32, numerator: u32, denominator: u32) -> f32 {
    first + (second - first) * numerator as f32 / denominator as f32
}