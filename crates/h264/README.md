# filmcraft-h264

Clean-room, pure-Rust (no `unsafe`) H.264 / AVC decoder, implemented from the public ITU-T
Rec. H.264 (ISO/IEC 14496-10) specification. It is an L0 crate: it depends only on
`filmcraft-bitstream`, `thiserror`, and optionally `rayon` (default feature `threads`), and builds
for `wasm32-unknown-unknown`.

All syntax tables (CAVLC code tables, the 1024 CABAC `(m, n)` context initialisation pairs,
`rangeTabLPS`, scaling defaults, deblocking thresholds, the 8x8 significance-map tables) were
extracted from the specification text (the extraction scripts only read the ITU-T PDF). No code was
taken or ported from FFmpeg, x264, JM or openh264. ffmpeg/libx264 are only used as external test
oracles and fixture generators.

## Supported

- Profiles: Constrained Baseline / Baseline, Main, High, High 10 and High 4:2:2 (and their Intra
  variants): 4:2:0 and 4:2:2, 8 to 10 bits (luma and chroma of the same depth).
- NAL units: Annex-B byte streams and length-prefixed (`avcC`) samples; SPS (all fields incl.
  VUI/HRD, scaling lists with fall-back rules A/B, cropping, high-profile fields), PPS (incl.
  `transform_8x8_mode`, scaling lists, slice groups parsed), SEI / AUD / filler skipped.
- Slice header: all fields incl. reference list modification, prediction weight table and
  decoded reference picture marking. POC types 0, 1, 2. `frame_num` gap handling ("non-existing"
  frames).
- Entropy coding: CAVLC and CABAC (all syntax elements needed for frame coding, 4:2:0 and 4:2:2;
  the 2x4 chroma DC with nC = -2 tables and `NumC8x8` = 2 context selection).
- Macroblocks: I (Intra 4x4 / 8x8 / 16x16, I_PCM), P (all partitions, P_8x8ref0, P_Skip),
  B (all partitions and sub-partitions, B_Skip, B_Direct_16x16, B_Direct_8x8), spatial and temporal
  direct prediction with/without `direct_8x8_inference`, constrained intra prediction.
- Transforms / quantisation: 4x4, 8x8, Intra16x16 DC Hadamard, 2x2 (4:2:0) and 2x4 (4:2:2, QP'C + 3)
  chroma DC; flat and custom scaling matrices (SPS and PPS level); `QpBdOffset` in every QP
  derivation (QPY down to -12 at 10 bits, `mb_qp_delta` range and wrap).
- Intra prediction: all modes, 8x8 and 8x16 (4:2:2) chroma including the 4:2:2 DC and plane rules;
  unavailable neighbours at the bit depth's mid level.
- Inter prediction: quarter-sample 6-tap luma, eighth-sample chroma (4:2:2: full-height chroma with
  quarter-sample vertical vectors), explicit (offsets scaled to the bit depth) and implicit
  weighted prediction, multiple reference frames, long-term references.
- I_PCM at any bit depth (BitDepth bits per sample, MbHeightC chroma rows).
- DPB: sliding window, all MMCOs (1-6), IDR / `no_output_of_prior_pics`, bumping with DPB size from
  the level (or VUI `max_dec_frame_buffering`) and `max_num_reorder_frames`.
- Deblocking filter: full bS derivation, `disable_deblocking_filter_idc` 0/1/2, slice offsets,
  8x8-transform edges; alpha / beta / tC0 scaled to the bit depth; the four horizontal chroma
  edges of 4:2:2 (bS of the luma edge at the same row, also where the 8x8 transform leaves that luma
  edge unfiltered).
- Output: cropped planar 4:2:0 or 4:2:2 pictures in output order (`Plane::U8` at 8 bits,
  `Plane::U16` above; `Picture::bit_depth`, `Picture::chroma_format`) with the caller's `pts`, POC,
  key flag, VUI colour description (range, primaries, transfer, matrix) and sample aspect ratio.
- Frame-level multithreading (see below).
- Draft mode for reduced-resolution playback (off by default): non-reference pictures skip the
  deblocking filter and are flagged `Picture::draft`; every other picture stays bit-exact.

## API

```rust
use filmcraft_h264::Decoder;

let mut dec = Decoder::new(); // or Decoder::with_threads(n), Decoder::from_avcc(&avcc)?
// dec.threads(): worker threads in use; dec.set_draft(true): draft mode (playback only)
for (pts, access_unit) in access_units {
    for pic in dec.decode(access_unit, pts)? {
        // pic.width / pic.height (cropped), pic.y / pic.u / pic.v, pic.y_stride / pic.uv_stride,
        // pic.pts, pic.poc, pic.key, pic.color, pic.sar
    }
}
for pic in dec.flush() { /* ... */ }
if let Some(err) = dec.take_error() { /* error reported by a decoding thread */ }
```

`decode` expects whole access units (a buffer with several complete access units — e.g. an
entire Annex-B file — also works). `Decoder::stats()` returns counters of the coding tools seen
(macroblock types, slice types, MMCOs, long-term marks, frame_num gaps, ...).

### Threading model

The calling thread parses headers and performs all POC / DPB / reference-list bookkeeping (it only
needs header data). The macroblock layer of each picture is decoded as a job on a rayon pool. A job
reconstructs into private buffers, deblocks each macroblock row once the row below it is
reconstructed, and publishes finished rows (samples + motion data for co-located lookups) into the
picture's shared `Frame` (`OnceLock` per macroblock row). Jobs of later pictures block per row on
exactly the reference rows they read, so many pictures decode concurrently while the output stays
bit-exact. Without the `threads` feature (or on wasm, or with `with_threads(1)`) jobs run inline.
With threads, `decode` returns pictures once their job has finished, so output lags input by up to
roughly the number of threads.

Deblocking is row-parallel in this pipelined form: each picture's job filters macroblock row r as
soon as row r + 1 is reconstructed (intra prediction of row r + 1 reads the unfiltered bottom
line of row r) and publishes it one row later, while the jobs of other pictures run on other
threads. A picture waiting on a reference therefore waits only for that reference's rows its
motion vectors reach plus about two rows, so a chain of reference pictures (a seek from an IDR
picture) advances a few rows behind its predecessor instead of a whole picture behind it. A
separate deblocking thread per picture would shorten that lag by about one row and would need its
own copy of the reconstructed rows and macroblock state; it is not worth it while the pool has
more pictures in flight than cores. The hot loops are written to vectorise without `unsafe` or
intrinsics: the edge filters process all 16 (luma) or 8 (chroma) lines of an edge at once with
per-lane masks, the inverse transforms do their column pass across all columns, and the CABAC
engine decodes a context-coded bin without branching on its outcome.

### Draft mode

`Decoder::set_draft(true)` (reduced-resolution playback only; FilmCraft enables it with Settings ▸
Playback ▸ Draft decoding while the Program monitor plays at 1/2 or 1/4 resolution) skips the
deblocking filter (8.7) of non-reference pictures (`nal_ref_idc` 0). No picture is predicted from
a non-reference picture, and deblocking never changes the motion data that direct prediction
reads, so every other picture is unchanged; the skipped pictures are flagged `Picture::draft` and
the media stack never hands them to a paused monitor, a render or an export. On the 4K fixture
below it saves ~9 % of the decoding work (the B-pyramid's non-reference half of the B pictures).

## Tests

`cargo test -p filmcraft-h264` (fixtures are generated on first use into `target/fixtures/h264/`
with ffmpeg + libx264; tests print a message and skip when `/opt/homebrew/bin/ffmpeg` is absent).

- Unit tests: CAVLC tables (prefix-freeness / Kraft sums, spec-style residual example), CABAC engine
  (context init formula, round trip against an encoder model), intra predictors with hand-computed
  values, transforms, luma interpolation against a straightforward reference implementation,
  weighting, macroblock type tables, scaling-list parsing.
- Synthetic streams (`src/synth_tests.rs`): hand-built CAVLC and CABAC bitstreams covering what
  libx264 never produces — I_PCM (also 10-bit 4:2:2), long-term references, MMCO 1-6, `frame_num` gaps, reference list
  modification (short- and long-term), explicit weighted prediction in P and B slices, B_Skip with
  explicit bi-prediction. The expected output is known exactly and is also checked against ffmpeg.
- Robustness: avcC input with pts round trip; randomly corrupted, truncated and garbage input never
  panics or deadlocks (single- and multi-threaded).
- Conformance (`tests/it/conformance.rs`): every fixture is decoded single-threaded, with 3 threads
  and with every core, and compared sample-for-sample with `ffmpeg -f rawvideo` in the fixture's
  format (`yuv420p`, `yuv420p10le`, `yuv422p` or `yuv422p10le`; the picture's bit depth and chroma
  format must match too); on mismatch the first differing frame, plane, sample position and
  macroblock are reported. Output
  pts/POC order is checked too, and that no picture is flagged draft without draft mode. Draft
  mode (`draft_mode_changes_only_flagged_non_reference_pictures`): on five B-pyramid / B-frame
  fixtures, single- and frame-threaded, every unflagged picture is bit-exact and only flagged
  pictures differ.
- Vectorised kernels against their straightforward forms on random input: the deblocking edge
  filter against the per-line filter, the 4x4 / 8x8 inverse transforms against the per-column
  formulation. 10-bit kernels (clipping to 1023, scaled deblocking thresholds, 4:2:2 chroma DC and
  intra rules) against hand-computed values.

### Fixture matrix (all bit-exact, single- and multi-threaded)

libx264 fixtures unless noted; the VideoToolbox (hardware encoder) fixtures are skipped where that
encoder is unavailable.

| fixture | source / size | configuration |
|---|---|---|
| intra_cavlc | testsrc2 176x144 | Baseline, keyint=1, no deblock |
| intra_cavlc_noise | mandelbrot+noise 352x288 | Baseline, keyint=1, no deblock |
| intra_cavlc_8x8 | testsrc2+noise 352x288 | High CAVLC, 8x8 transform, intra only |
| intra_cavlc_cqm | testsrc2+noise 352x288 | High CAVLC, cqm=jvt, intra only |
| p_cavlc_nodeblock | testsrc2+noise 352x288 | Baseline, ref=3, no deblock |
| baseline_qcif | testsrc2 176x144 | Baseline, keyint=15 |
| baseline_cif_noise | mandelbrot+noise 352x288 | Baseline |
| cavlc_b | testsrc2+noise 352x288 | High, cabac=0, bframes=3 |
| cavlc_b_temporal | testsrc2+noise 352x288 | Main, cabac=0, direct=temporal, weightb |
| cavlc_weightp | testsrc2 fade 352x288 | Main, cabac=0, weightp=2, weightb |
| slices4_cavlc | testsrc2+noise 352x288 | Baseline, slices=4 |
| qp1_cavlc | mandelbrot+noise 176x144 | Main, CAVLC, QP 1 |
| main_cabac_b | testsrc2+noise 352x288 | Main (CABAC, B-frames) |
| high_720p | smptehdbars+noise 1280x720 | High |
| high_1080p | testsrc2 1920x1080 | High |
| crop_1918x1078 | testsrc2+noise 1918x1078 | High, frame cropping |
| bpyramid | testsrc2+noise 352x288 | bframes=3, b-pyramid=normal (MMCO) |
| weightp2 | testsrc2 fade 352x288 | weightp=2 |
| weightb | testsrc2 fade 352x288 | weightb=1, bframes=3 (implicit) |
| direct_temporal | testsrc2+noise 352x288 | direct=temporal |
| direct_spatial | mandelbrot+noise 352x288 | direct=spatial |
| ref4 | testsrc2+noise 352x288 | ref=4 |
| no_deblock | testsrc2+noise 352x288 | no-deblock |
| deblock_m2_2 | testsrc2+noise 352x288 | deblock=-2,2 |
| cqm_jvt | testsrc2+noise 352x288 | cqm=jvt |
| slices4 | testsrc2+noise 352x288 | slices=4 (CABAC) |
| keyint10 | testsrc2+noise 352x288 | keyint=10 |
| open_gop | testsrc2+noise 352x288 | open-gop, keyint=12, bframes=3 |
| constrained_intra | testsrc2+noise 352x288 | constrained-intra |
| no8x8dct | testsrc2+noise 352x288 | 8x8dct=0 |
| qp50 | testsrc2+noise 352x288 | QP 50 |
| qp1 | mandelbrot+noise 352x288 | QP 1 |
| high10_intra | testsrc2+noise 352x288 | High 10 (4:2:0 10-bit), keyint=1 |
| high10_cavlc_intra | testsrc2+noise 352x288 | High 10, CAVLC, keyint=1 |
| high10_b | testsrc2+noise 352x288 | High 10, bframes=3, b-pyramid, ref=3 |
| high10_cavlc_b | mandelbrot+noise 352x288 | High 10, CAVLC, bframes=3, direct=temporal |
| high10_weightp | testsrc2 fade 352x288 | High 10, weightp=2, weightb |
| high10_qp1 | mandelbrot+noise 176x144 | High 10, QP 1 (QPY -11) |
| high10_qp60 | testsrc2+noise 352x288 | High 10, QP 60 |
| high10_cqm_crop | testsrc2+noise 350x286 | High 10, cqm=jvt, chroma-qp-offset=-3, cropping |
| high422_intra | testsrc2+noise 352x288 | High 4:2:2 (10-bit), keyint=1 |
| high422_cavlc_intra | smptehdbars+noise 352x288 | High 4:2:2 10-bit, CAVLC, keyint=1 |
| high422_b | testsrc2+noise 352x288 | High 4:2:2 10-bit, bframes=3, weightp=2, weightb |
| high422_cavlc_b | mandelbrot+noise 352x288 | High 4:2:2 10-bit, CAVLC, bframes=3, 8x8 transform |
| high422_8bit | testsrc2+noise 352x288 | High 4:2:2 at 8 bits, bframes=2 |
| high422_qp55 | smptehdbars+noise 352x288 | High 4:2:2 10-bit, QP 55 |
| bench_1080p | testsrc2+noise 1920x1080, 120 frames | preset medium, CRF 20 (8.8 Mbit/s) |
| vt_baseline | testsrc2+noise 320x240 | Apple VideoToolbox, Baseline (macOS only) |
| vt_main | mandelbrot+noise 640x360 | VideoToolbox, Main |
| vt_high | testsrc2+noise 640x360 | VideoToolbox, High |
| vt_high_1080p | testsrc2 1920x1080 | VideoToolbox, High 8 Mbit/s |

`cargo test --release -p filmcraft-h264 --test it conformance::coverage_report -- --ignored --nocapture`
prints which coding tools each fixture exercises.

## Performance

`cargo test --release -p filmcraft-h264 --test it perf:: -- --ignored --nocapture` (1080p High profile,
CABAC, B-pyramid, 120 frames, 8.8 Mbit/s; bit-exactness is verified first). Measured on an Apple M4
Pro (14 cores) while the machine was heavily shared (load average 40-180), so these are lower bounds:

| threads | fps |
|---|---|
| 1 | ~105-122 (≈ 33 Mcycles per frame) |
| 14 | ~500-600 |

4K High profile (testsrc2 + grain, 3840x2160, CABAC, B-pyramid, ~170 Mbit/s; the first 48 frames
of the playback benchmark's `a2160.mp4`), CPU cycles per frame counted by the kernel
(`proc_pid_rusage`; independent of machine load, all threads summed; bit-exact with ffmpeg):

| build | Mcycles / frame | instructions / frame |
|---|---|---|
| before M4.9 | 505 | 1232 M |
| M4.9 | **390** (−23 %) | 1195 M |
| M4.9, draft mode | **356** (−30 %) | 1068 M |

The same on the benchmark's 1080p fixture (`dec_h264_1080.mp4`, 120 frames): 132 → 100 Mcycles
per frame (−24 %), 90 in draft mode. Single-threaded 4K time (`sample` profile) splits into CABAC
residual decoding 41 %, deblocking 16 % (25-30 % before), other macroblock-layer parsing 11 %,
inverse transforms and dequantisation 9 %, motion compensation 8 %, intra prediction 6 %, row and
picture copies 4 %. At ~4.5 GHz a 4K frame takes ~85 ms of one
core, so 23.976 fps needs about two cores' worth of frame threads.

The example `cargo run --release -p filmcraft-h264 --example h264dec -- in.h264 out.yuv` decodes a
file; `H264_BENCH_ITERS=n` (and `H264_THREADS=t`, `H264_DRAFT=1`) turns it into a benchmark,
`H264_STATS=1` prints the coverage counters.

## Known gaps

- Interlaced coding (field pictures, PAFF, MBAFF): streams with `frame_mbs_only_flag = 0` return
  `Error::Unsupported`.
- 4:4:4 (High 4:4:4 Predictive, separate colour planes), monochrome (4:0:0), bit depths above 10,
  luma and chroma of different depths, and lossless (`qpprime_y_zero_transform_bypass`) return
  `Error::Unsupported`.
- FMO (slice groups), SP/SI slices, data partitioning (Extended profile), MVC/SVC NAL units.
- Error concealment is minimal: undecodable slices leave the prediction/previous content; missing
  references are replaced by other references.
- No SIMD intrinsics (the hot loops are written to auto-vectorise); CABAC residual decoding is
  inherently serial and now dominates.

### High bit depth and 4:2:2

Reconstruction is generic over the stored sample type (`sample::Sample`: `u8` for 8-bit
sequences, `u16` for 9 and 10 bits) and monomorphised, so the 8-bit path runs exactly the code it
did before (same speed). The DPB keeps `AnyFrame` (either type); a sequence of another bit depth or
chroma format flushes it, and each picture job runs the matching core. 10 bits is the limit
because the deblocking lanes are `i16` and the chroma interpolation sum is 16-bit
(64 x 1023 + 32 < 65536). The chroma geometry follows `MbHeightC` (8 or 16) throughout: planes,
published rows, chroma windows, intra prediction, residual block counts and CABAC / CAVLC
neighbours.
