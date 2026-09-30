//! Every built-in live profile is previewed on hover in the UI (`keylume_live::preview`,
//! an 8 s clip at 20 fps). A preview that barely changes or sits near-black looks dead
//! or frozen, so every one of them must visibly move and never be mostly dark.

use keylume_live::{preview, LiveFrame};
use keylume_profiles::Lighting;

const PREVIEW_SECONDS: f32 = 8.0;
const PREVIEW_FPS: f32 = 20.0;
/// Largest allowed change from the first frame before a preview counts as "moving".
const MIN_SPAN: f32 = 0.08;
/// Minimum mean brightness before a preview counts as "not dark".
const MIN_MEAN: f32 = 0.08;

fn luminance(f: &LiveFrame) -> f32 {
    match f {
        LiveFrame::Color(c) => (c.0 as f32 * 0.3 + c.1 as f32 * 0.59 + c.2 as f32 * 0.11) / 255.0,
        LiveFrame::Levels(l) => l.iter().map(|&v| v as f32).sum::<f32>() / (6.0 * l.len() as f32),
    }
}

fn diff(a: &LiveFrame, b: &LiveFrame) -> f32 {
    match (a, b) {
        (LiveFrame::Color(x), LiveFrame::Color(y)) => {
            ((x.0 as f32 - y.0 as f32).abs() + (x.1 as f32 - y.1 as f32).abs() + (x.2 as f32 - y.2 as f32).abs()) / 765.0
        }
        (LiveFrame::Levels(x), LiveFrame::Levels(y)) => x.iter().zip(y).map(|(a, b)| (*a as f32 - *b as f32).abs()).sum::<f32>() / (6.0 * 32.0),
        _ => 1.0,
    }
}

#[test]
fn every_live_profile_preview_moves_and_is_not_dark() {
    let layout = keylume_proto::Layout::tk68();
    let lib = keylume_profiles::builtin(&layout);

    let mut checked = 0;
    let mut failures = Vec::new();
    for p in &lib {
        let Lighting::Live { live } = &p.lighting else { continue };
        checked += 1;
        let frames = preview(live.clone(), PREVIEW_SECONDS, PREVIEW_FPS);
        let span = frames.iter().map(|f| diff(&frames[0], f)).fold(0.0f32, f32::max);
        let mean = frames.iter().map(luminance).sum::<f32>() / frames.len() as f32;
        if span < MIN_SPAN {
            failures.push(format!("{} [{}]: preview barely moves (span {span:.3})", p.name, p.category));
        }
        if mean < MIN_MEAN {
            failures.push(format!("{} [{}]: preview is too dark (mean {mean:.3})", p.name, p.category));
        }
    }

    assert!(checked > 0, "no live profiles found in the built-in library");
    assert!(failures.is_empty(), "{} preview(s) look frozen or dark:\n{}", failures.len(), failures.join("\n"));
}
