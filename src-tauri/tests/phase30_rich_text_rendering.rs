//! Phase 30 — Rich-Text Rendering & Parity acceptance tests.
//!
//! These tests exercise the shared production renderer, its preview packet and
//! the headless export seam described by `Plans/plan_phase30_rich_text_rendering.md`.

use std::fs;
use std::path::{Path, PathBuf};
use tinixcut_lib::modules::composition::{CompositionPlan, TextOverlayInput};
use tinixcut_lib::modules::domain::text::{
    FontReference, GradientKind, GradientSpace, GradientStop, RichTextSpan, StrokeAlign,
    TextBackgroundBox, TextFill, TextLayout, TextRichStyle, TextShadow, TextSpanStyle, TextStroke,
    TextWrapMode,
};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    ItemKind, Keyframe, KeyframeProperty, MediaKind, Project, TextAnchor, TextStyle, TimelineItem,
    Track, TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::resolver::MediaBinaryResolver;
use tinixcut_lib::modules::media::text_overlay::{
    preview_overlay_at, render_frame_overlay_buffer_at, render_overlay_for_layer, resolve_font,
    OverlayPlacement, OverlayStyle, ResolvedFont, RichTextRenderer, TextOverlayError,
    TEXT_OVERLAY_RENDERER_VERSION,
};
use tinixcut_lib::modules::media::text_renderer_poc::{ColorRgba, RgbaBuffer};
use tinixcut_lib::modules::media::MediaCatalog;

const BUNDLED_FONT_SHA256: &str =
    "61F89F8DB49261C2F6106E8DCCC35DF7B2F7ED909020DB40A3FC905E95F99334";

fn write_evidence(dir: &Path, name: &str, bytes: &[u8]) {
    let path = dir.join(name);
    let mut last_error = None;
    for _ in 0..40 {
        match fs::write(&path, bytes) {
            Ok(()) => return,
            Err(error) => {
                last_error = Some(error);
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        }
    }
    panic!("failed to write evidence {name}: {last_error:?}");
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_phase30_{name}"));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir).expect("create test dir");
    dir
}

fn solid(color: &str) -> TextFill {
    TextFill::Solid {
        color: color.to_string(),
    }
}

fn base_rich(color: &str) -> TextRichStyle {
    TextRichStyle {
        font: None,
        font_size: Some(96.0),
        layout: TextLayout::default(),
        fill: solid(color),
        strokes: Vec::new(),
        shadows: Vec::new(),
        background: None,
        spans: Vec::new(),
    }
}

fn project_with_rich_text(
    name: &str,
    text: &str,
    rich: Option<TextRichStyle>,
) -> (Project, String) {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new(name.to_string());
    project.settings.frame_rate = rate;
    project.settings.width = 1920;
    project.settings.height = 1080;

    let mut track = Track::new("Text Track".to_string(), TrackKind::Video);
    let mut item = TimelineItem::new(
        None,
        "Rich Text".to_string(),
        ItemKind::Text,
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(60, rate),
        ),
        TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(60, rate),
        ),
    )
    .expect("create text item");
    item.text_content = Some(text.to_string());
    item.text_rich = rich;
    let item_id = item.item_id.clone();
    track.items.push(item);
    project.tracks.push(track);
    project.validate().expect("project is canonical");
    (project, item_id)
}

fn render(
    rich: &TextRichStyle,
    text: &str,
) -> tinixcut_lib::modules::media::text_overlay::RichTextOverlay {
    let renderer = RichTextRenderer::new(1920, 1080, ResolvedFont::bundled());
    let style = OverlayStyle::from_rich(rich, None).expect("style");
    renderer
        .render_overlay(text, &style, &OverlayPlacement::default())
        .expect("render")
}

#[test]
fn p30_ac01_typography_layout_and_unicode_are_canonical() {
    // Line height and letter spacing must reach the shared renderer.
    let mut tall = base_rich("#FFFFFF");
    tall.layout.line_height = 2.0;
    let normal = render(&base_rich("#FFFFFF"), "Tinix\nEditor");
    let tall_overlay = render(&tall, "Tinix\nEditor");
    assert!(
        (tall_overlay.roi[3] - tall_overlay.roi[1]) > (normal.roi[3] - normal.roi[1]),
        "line_height must change rendered block height"
    );

    let mut spaced = base_rich("#FFFFFF");
    spaced.layout.letter_spacing = 8.0;
    let spaced_overlay = render(&spaced, "TRACKING");
    let base_overlay = render(&base_rich("#FFFFFF"), "TRACKING");
    assert!(
        (spaced_overlay.roi[2] - spaced_overlay.roi[0])
            > (base_overlay.roi[2] - base_overlay.roi[0]),
        "letter_spacing must change rendered width"
    );

    // NFC and NFD Vietnamese must normalize to identical pixels.
    let nfc = "Tiếng Việt";
    let nfd: String = nfc
        .chars()
        .flat_map(|character| {
            use unicode_normalization::UnicodeNormalization;
            character.to_string().nfd().collect::<Vec<_>>()
        })
        .collect();
    let nfc_overlay = render(&base_rich("#FFFFFF"), nfc);
    let nfd_overlay = render(&base_rich("#FFFFFF"), &nfd);
    assert_eq!(nfc_overlay.roi_png_bytes, nfd_overlay.roi_png_bytes);

    // Wrap/AutoFit and rich spans stay inside the authored max width.
    let mut wrap = base_rich("#FFFFFF");
    wrap.layout = TextLayout {
        max_width: Some(420.0),
        wrap_mode: TextWrapMode::Wrap,
        line_height: 1.2,
        ..TextLayout::default()
    };
    let wrapped = render(&wrap, "Tinix rich text wraps deterministically");
    assert!(wrapped.roi[2] - wrapped.roi[0] <= 430);

    wrap.layout.wrap_mode = TextWrapMode::AutoFit;
    let fitted = render(&wrap, "Tinix rich text wraps deterministically");
    assert!(fitted.roi[2] - fitted.roi[0] <= 430);

    let mut spans = base_rich("#FFFFFF");
    spans.spans = vec![RichTextSpan {
        start: 0,
        end: 3,
        style: TextSpanStyle {
            fill: Some(solid("#22DD88")),
            font_size: Some(150.0),
            letter_spacing: Some(2.0),
            ..TextSpanStyle::default()
        },
    }];
    let spanned = render(&spans, "ABCdef");
    assert!(spanned.has_visible_content);
    assert_ne!(
        spanned.roi_png_bytes,
        render(&base_rich("#FFFFFF"), "ABCdef").roi_png_bytes
    );

    // Unsupported scripts are an explicit diagnostic, never `.notdef` boxes.
    let error = RichTextRenderer::new(1920, 1080, ResolvedFont::bundled())
        .render_overlay(
            "漢字",
            &OverlayStyle::from_rich(&base_rich("#FFFFFF"), None).expect("style"),
            &OverlayPlacement::default(),
        )
        .expect_err("CJK must be diagnosed as missing glyphs");
    assert!(matches!(error, TextOverlayError::MissingGlyphs(_)));
}

#[test]
fn p30_ac02_font_resolution_is_deterministic_and_explicit() {
    let bundled = resolve_font(None, &[]).expect("bundled default");
    assert_eq!(bundled.sha256, BUNDLED_FONT_SHA256);
    assert_eq!(
        bundled.origin,
        tinixcut_lib::modules::media::text_overlay::FontOrigin::Bundled
    );

    let unknown = FontReference {
        family: "Definitely Not Bundled".to_string(),
        weight: 400,
        style: Default::default(),
        asset_id: None,
    };
    assert!(matches!(
        resolve_font(Some(&unknown), &[]),
        Err(TextOverlayError::MissingFont { .. })
    ));

    let dir = test_dir("font_asset");
    let source = Path::new("fixtures/fonts/Roboto-Bold.ttf");
    let target = dir.join("Roboto-Bold.ttf");
    fs::copy(source, &target).expect("copy font fixture");

    let mut project = Project::new("Font asset".to_string());
    let asset_id = MediaCatalog::register_asset(
        &mut project,
        None,
        "Roboto-Bold.ttf".to_string(),
        target.to_string_lossy().to_string(),
        MediaKind::Image,
        None,
        fs::metadata(&target).expect("metadata").len(),
        None,
    );
    let reference = FontReference {
        family: "Roboto".to_string(),
        weight: 700,
        style: Default::default(),
        asset_id: Some(asset_id.clone()),
    };
    let asset_font = resolve_font(Some(&reference), &project.assets).expect("asset font");
    assert_eq!(asset_font.sha256, BUNDLED_FONT_SHA256);
    assert_eq!(
        asset_font.origin,
        tinixcut_lib::modules::media::text_overlay::FontOrigin::Asset(asset_id)
    );

    // Removing the file must fail explicitly, never substitute another face.
    fs::remove_file(&target).expect("remove font");
    assert!(matches!(
        resolve_font(Some(&reference), &project.assets),
        Err(TextOverlayError::MissingFont { .. })
    ));

    let junk = dir.join("junk.ttf");
    fs::write(&junk, b"not a font").expect("write junk font");
    let junk_asset_id = MediaCatalog::register_asset(
        &mut project,
        None,
        "junk.ttf".to_string(),
        junk.to_string_lossy().to_string(),
        MediaKind::Image,
        None,
        10,
        None,
    );
    let junk_reference = FontReference {
        family: "Junk".to_string(),
        weight: 400,
        style: Default::default(),
        asset_id: Some(junk_asset_id),
    };
    assert!(matches!(
        resolve_font(Some(&junk_reference), &project.assets),
        Err(TextOverlayError::InvalidFont(_))
    ));
}

#[test]
fn p30_ac04_ac05_style_primitives_and_alpha_are_shared() {
    let mut rich = base_rich("#FFFFFF");
    rich.fill = TextFill::Gradient {
        gradient_kind: GradientKind::Linear,
        angle_deg: 118.0,
        radius: None,
        space: GradientSpace::All,
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: "#FFE35F".to_string(),
            },
            GradientStop {
                offset: 1.0,
                color: "#FFFFFF".to_string(),
            },
        ],
    };
    rich.strokes = vec![
        TextStroke {
            color: "#4A2000".to_string(),
            width: 7.0,
            align: StrokeAlign::Outside,
        },
        TextStroke {
            color: "#AD5100".to_string(),
            width: 4.0,
            align: StrokeAlign::Outside,
        },
    ];
    rich.shadows = vec![
        TextShadow {
            color: "#AD5100E6".to_string(),
            offset_x: 8.0,
            offset_y: 8.0,
            blur: 0.0,
        },
        TextShadow {
            color: "#FFA758E6".to_string(),
            offset_x: 12.0,
            offset_y: 12.0,
            blur: 3.0,
        },
    ];
    rich.background = Some(TextBackgroundBox {
        fill: solid("#101010CC"),
        padding_x: 18.0,
        padding_y: 10.0,
        corner_radius: 12.0,
        opacity: 1.0,
    });

    let first = render(&rich, "Tinix");
    let second = render(&rich, "Tinix");
    assert!(first.has_visible_content);
    assert_eq!(
        first.roi_png_bytes, second.roi_png_bytes,
        "shared renderer output must be bit-for-bit deterministic"
    );

    // A pure white fill composited over white must not darken (no black halo).
    let renderer = RichTextRenderer::new(640, 360, ResolvedFont::bundled());
    let white_rich = base_rich("#FFFFFF");
    let style = OverlayStyle::from_rich(&white_rich, None).expect("style");
    let buffer = renderer
        .render_overlay_buffer("Halo", &style, &OverlayPlacement::default())
        .expect("buffer");

    let halo = check_no_dark_halo(&buffer, ColorRgba::new(1.0, 1.0, 1.0, 1.0));
    assert!(
        halo > 0,
        "expected visible white glyph pixels for the halo check"
    );

    // Transform placement keeps pixels inside the canonical canvas.
    let placement = OverlayPlacement {
        anchor: TextAnchor::TopLeft,
        offset_x: 80.0,
        offset_y: 120.0,
        scale_x: 1.35,
        scale_y: 1.35,
        rotation_deg: 21.0,
        opacity: 0.9,
    };
    let transformed = renderer
        .render_overlay("Rotated", &style, &placement)
        .expect("transform");
    assert!(transformed.has_visible_content);
    assert!(transformed.roi[2] <= 640 && transformed.roi[3] <= 360);
}

fn check_no_dark_halo(buffer: &RgbaBuffer, background: ColorRgba) -> usize {
    let mut visible = 0usize;
    for pixel in buffer.data.as_chunks::<4>().0 {
        let alpha = pixel[3] as f32 / 255.0;
        if alpha <= 0.01 || alpha >= 0.99 {
            continue;
        }
        visible += 1;
        let blended = pixel[0] as f32 / 255.0 * alpha + background.r * (1.0 - alpha);
        assert!(
            blended >= 0.98,
            "white glyph compositing produced a dark halo: {blended}"
        );
    }
    visible
}

#[test]
fn p30_ac04_preview_and_export_consume_identical_overlay_pixels() {
    let rich = base_rich("#FF3355");
    let (project, item_id) = project_with_rich_text("Parity", "Tiếng Việt", Some(rich));
    let timestamp = RationalTime::new(0, project.settings.frame_rate);

    let preview = preview_overlay_at(&project, &item_id, timestamp)
        .expect("preview render")
        .expect("active layer");
    let plan = CompositionPlan::from_project(&project).expect("composition");
    let layer = plan
        .visual_layers
        .iter()
        .find(|layer| layer.item_id == item_id)
        .expect("composition layer");
    let export = render_overlay_for_layer(&project, plan.width, plan.height, layer, layer.opacity)
        .expect("export render")
        .expect("rich layer");

    assert_eq!(preview.roi, export.roi);
    assert_eq!(preview.roi_png_bytes, export.roi_png_bytes);
    assert_eq!(preview.diagnostics, export.diagnostics);
    assert_eq!(
        preview.diagnostics.renderer_version,
        TEXT_OVERLAY_RENDERER_VERSION
    );

    // Headless export must reference the materialized overlay and never drawtext.
    let dir = test_dir("export_parity");
    let png_path = dir.join(format!("{item_id}.png"));
    fs::write(&png_path, &export.roi_png_bytes).expect("write overlay png");
    let overlay = tinixcut_lib::modules::composition::TextOverlayInput {
        item_id: item_id.clone(),
        path: png_path.clone(),
        roi: export.roi,
    };
    let (args, filtergraph) = plan.to_ffmpeg_export_args_with_overlays(
        &dir.join("out.mp4"),
        23,
        "libx264",
        None,
        &[overlay],
    );
    assert!(filtergraph.contains(&format!("overlay=x={}:y={}", export.roi[0], export.roi[1])));
    assert!(!filtergraph.contains("drawtext"));
    assert!(args
        .iter()
        .any(|argument| argument == &png_path.to_string_lossy().to_string()));

    // Legacy text items keep the legacy drawtext path unchanged.
    let (legacy_project, _legacy_id) = project_with_rich_text("Legacy", "Legacy title", None);
    let legacy_plan = CompositionPlan::from_project(&legacy_project).expect("legacy plan");
    let (legacy_filter, _) = legacy_plan.build_video_filtergraph(&[], 1.0, None);
    assert!(legacy_filter.contains("drawtext=text='Legacy title'"));
}

#[test]
fn phase30_legacy_rounded_caption_preview_and_mp4_pixels_match() {
    let (mut project, item_id) =
        project_with_rich_text("Legacy rounded caption", "Tinix Cut", None);
    project.settings.width = 320;
    project.settings.height = 180;

    let rate = project.settings.frame_rate;
    let short_range = TimeRange::new(RationalTime::zero(rate), RationalTime::from_frames(6, rate));
    let item = &mut project.tracks[0].items[0];
    item.timeline_range = short_range;
    item.source_range = short_range;
    item.text_style = Some(TextStyle {
        font_family: "Inter".to_string(),
        font_size: 48.0,
        font_weight: 700,
        font_color: "#FFFFFF".to_string(),
        background_color: Some("#E02020".to_string()),
        background_opacity: 0.85,
        background_corner_radius: 24.0,
        padding_x: 22.0,
        padding_y: 12.0,
        ..TextStyle::default()
    });
    project.validate().expect("rounded legacy caption is valid");
    let timestamp = RationalTime::zero(rate);

    // Exercise the UI-supported radius range at representative values and ensure
    // preview and export materialize the same rounded pixels for each value.
    for radius in [4.0, 8.0, 14.0, 24.0, 48.0] {
        project.tracks[0].items[0]
            .text_style
            .as_mut()
            .expect("legacy text style")
            .background_corner_radius = radius;
        project.validate().expect("rounded caption radius is valid");

        let preview = preview_overlay_at(&project, &item_id, timestamp)
            .expect("preview render at tested radius")
            .expect("active caption at tested radius");
        let plan =
            CompositionPlan::from_project(&project).expect("composition plan at tested radius");
        let layer = plan
            .visual_layers
            .iter()
            .find(|layer| layer.item_id == item_id)
            .expect("caption layer at tested radius");
        let export_overlay =
            render_overlay_for_layer(&project, plan.width, plan.height, layer, layer.opacity)
                .expect("export overlay render at tested radius")
                .expect("rounded caption export overlay at tested radius");

        assert_eq!(preview.roi, export_overlay.roi, "radius {radius} px");
        assert_eq!(
            preview.roi_png_bytes, export_overlay.roi_png_bytes,
            "preview/export pixels differ at radius {radius} px"
        );
        let pixels = RgbaBuffer::from_png_bytes(&preview.roi_png_bytes)
            .expect("decode rounded overlay at tested radius");
        let corner = ((pixels.height - 1) as usize * pixels.width as usize) * 4;
        assert_eq!(
            pixels.data[corner + 3],
            0,
            "rounded corner must remain transparent at radius {radius} px"
        );
    }

    // Keep the end-to-end MP4 pixel check at the normal 24 px preset.
    project.tracks[0].items[0]
        .text_style
        .as_mut()
        .expect("legacy text style")
        .background_corner_radius = 24.0;
    let preview = preview_overlay_at(&project, &item_id, timestamp)
        .expect("preview render")
        .expect("active caption");
    let plan = CompositionPlan::from_project(&project).expect("composition plan");
    let layer = plan
        .visual_layers
        .iter()
        .find(|layer| layer.item_id == item_id)
        .expect("caption layer");
    assert!(layer.uses_shared_text_overlay());

    let export_overlay =
        render_overlay_for_layer(&project, plan.width, plan.height, layer, layer.opacity)
            .expect("export overlay render")
            .expect("rounded legacy caption overlay");
    assert_eq!(preview.roi, export_overlay.roi);
    assert_eq!(preview.roi_png_bytes, export_overlay.roi_png_bytes);

    let overlay_pixels = RgbaBuffer::from_png_bytes(&preview.roi_png_bytes)
        .expect("decode shared preview/export overlay");
    let corner = ((overlay_pixels.height - 1) as usize * overlay_pixels.width as usize) * 4;
    let top_center = (overlay_pixels.width as usize / 2) * 4;
    assert_eq!(
        overlay_pixels.data[corner + 3],
        0,
        "rounded corner stays transparent"
    );
    assert!(
        overlay_pixels.data[top_center + 3] > 150,
        "top center contains the caption background"
    );

    let dir = test_dir("legacy_rounded_caption_export");
    let output = dir.join("legacy_rounded_caption.mp4");
    let profile = ExportProfile::from_project_settings(&project.settings);
    let report = ExportEngine::render_snapshot(&project, &output, &profile, None)
        .expect("export rounded legacy caption");
    assert!(report.file_size_bytes > 0);

    let frame_bytes = MediaBinaryResolver::command("ffmpeg")
        .expect("ffmpeg available")
        .args([
            "-v",
            "error",
            "-i",
            &output.to_string_lossy(),
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])
        .output()
        .expect("decode exported frame");
    assert!(frame_bytes.status.success());
    let frame = RgbaBuffer {
        width: project.settings.width,
        height: project.settings.height,
        data: frame_bytes.stdout,
    };
    let [roi_x, roi_y, _, _] = preview.roi;
    let corner_offset = ((roi_y as usize * frame.width as usize) + roi_x as usize) * 4;
    let background_x = roi_x + overlay_pixels.width / 2;
    let background_y = roi_y + 2;
    let background_offset =
        ((background_y as usize * frame.width as usize) + background_x as usize) * 4;
    assert!(
        frame.data[background_offset] > frame.data[corner_offset].saturating_add(80),
        "MP4 must preserve the red caption background and leave its rounded corner clear"
    );

    fs::remove_dir_all(&dir).expect("remove temporary export artifacts");
}

#[test]
fn g53_keyframed_square_background_legacy_text_uses_shared_frame_renderer() {
    let (mut project, item_id) = project_with_rich_text("G53 legacy text", "Caption", None);
    project.settings.width = 320;
    project.settings.height = 180;
    let rate = project.settings.frame_rate;
    let item = &mut project.tracks[0].items[0];
    item.text_style = Some(TextStyle {
        font_size: 34.0,
        background_color: Some("#E02020".to_string()),
        background_opacity: 1.0,
        background_corner_radius: 0.0,
        ..TextStyle::default()
    });
    item.transform.position_y = -100.0;
    item.keyframes = vec![
        Keyframe::new(KeyframeProperty::PositionY, RationalTime::zero(rate), 55.0),
        Keyframe::new(
            KeyframeProperty::PositionY,
            RationalTime::from_frames(4, rate),
            -40.0,
        ),
    ];
    project
        .validate()
        .expect("G53 legacy text project is valid");

    let plan = CompositionPlan::from_project(&project).expect("composition plan");
    let layer = plan
        .visual_layers
        .iter()
        .find(|layer| layer.item_id == item_id)
        .expect("legacy text layer");
    assert!(layer.uses_shared_text_overlay());

    let mut frame_rois = Vec::new();
    for frame in [0, 4] {
        let timestamp = RationalTime::from_frames(frame, rate);
        let preview = preview_overlay_at(&project, &item_id, timestamp)
            .expect("preview uses shared legacy renderer")
            .expect("active text layer");
        let full_frame = render_frame_overlay_buffer_at(&project, &plan, layer, timestamp)
            .expect("per-frame export overlay")
            .expect("shared overlay exists");
        let exported_roi = full_frame.crop(preview.roi).to_png_bytes();
        assert_eq!(
            preview.roi_png_bytes, exported_roi,
            "preview ROI matches full-canvas export buffer at frame {frame}"
        );
        assert!(preview.roi[2] - preview.roi[0] > 1, "ROI has visible width");
        assert!(
            preview.roi[3] - preview.roi[1] > 1,
            "ROI has visible height"
        );
        frame_rois.push(preview.roi);
    }
    assert_ne!(
        frame_rois[0], frame_rois[1],
        "position keyframes move the ROI"
    );

    let sequence_overlay = TextOverlayInput {
        item_id: item_id.clone(),
        path: PathBuf::from("g53_frame_%06d.png"),
        roi: [0, 0, 320, 180],
    };
    let (_, export_filtergraph) = plan.to_ffmpeg_export_args_with_overlays(
        Path::new("g53_export.mp4"),
        23,
        "libx264",
        None,
        &[sequence_overlay],
    );
    assert!(
        export_filtergraph.contains("overlay=x=0:y=0"),
        "full export consumes the per-frame shared-renderer sequence"
    );
    assert!(
        !export_filtergraph.contains("drawtext"),
        "keyframed legacy text must not fall back to static drawtext"
    );

    // Static legacy square-background text remains on the established drawtext route.
    let (mut static_project, _) = project_with_rich_text("Static legacy", "Legacy title", None);
    static_project.tracks[0].items[0].text_style = Some(TextStyle {
        background_color: Some("#E02020".to_string()),
        background_opacity: 1.0,
        background_corner_radius: 0.0,
        ..TextStyle::default()
    });
    let static_plan = CompositionPlan::from_project(&static_project).expect("static plan");
    let static_layer = static_plan.visual_layers.first().expect("static layer");
    assert!(!static_layer.uses_shared_text_overlay());
    let (filter, _) = static_plan.build_video_filtergraph(&[], 1.0, None);
    assert!(filter.contains("drawtext=text='Legacy title'"));
}

#[test]
fn g53_keyframed_legacy_text_without_background_uses_shared_frame_renderer() {
    let (mut project, item_id) =
        project_with_rich_text("G53 animated legacy text", "Caption", None);
    project.settings.width = 320;
    project.settings.height = 180;
    let rate = project.settings.frame_rate;
    let item = &mut project.tracks[0].items[0];
    item.text_style = Some(TextStyle::default());
    item.keyframes = vec![
        Keyframe::new(KeyframeProperty::PositionY, RationalTime::zero(rate), 55.0),
        Keyframe::new(
            KeyframeProperty::PositionY,
            RationalTime::from_frames(4, rate),
            -40.0,
        ),
    ];
    project
        .validate()
        .expect("animated legacy project is valid");

    let plan = CompositionPlan::from_project(&project).expect("composition plan");
    let layer = plan
        .visual_layers
        .iter()
        .find(|layer| layer.item_id == item_id)
        .expect("legacy text layer");
    assert!(layer.uses_shared_text_overlay());

    for frame in [0, 4] {
        let timestamp = RationalTime::from_frames(frame, rate);
        let preview = preview_overlay_at(&project, &item_id, timestamp)
            .expect("preview uses shared legacy renderer")
            .expect("active text layer");
        let full_frame = render_frame_overlay_buffer_at(&project, &plan, layer, timestamp)
            .expect("per-frame export overlay")
            .expect("shared overlay exists");
        assert_eq!(
            preview.roi_png_bytes,
            full_frame.crop(preview.roi).to_png_bytes(),
            "no-background preview ROI matches export at frame {frame}"
        );
        assert!(preview.roi[2] - preview.roi[0] > 1);
        assert!(preview.roi[3] - preview.roi[1] > 1);
    }

    // Preview opacity remains a CSS property; the export frame buffer bakes it
    // into alpha. Verify the export alpha matches half-opacity for the same text.
    let mut half_opacity_project = project.clone();
    half_opacity_project.tracks[0].items[0].keyframes.extend([
        Keyframe::new(KeyframeProperty::Opacity, RationalTime::zero(rate), 0.5),
        Keyframe::new(
            KeyframeProperty::Opacity,
            RationalTime::from_frames(4, rate),
            0.5,
        ),
    ]);
    half_opacity_project
        .validate()
        .expect("half-opacity keyframes are valid");
    let opacity_plan =
        CompositionPlan::from_project(&half_opacity_project).expect("half-opacity plan");
    let opacity_layer = opacity_plan
        .visual_layers
        .iter()
        .find(|layer| layer.item_id == item_id)
        .expect("half-opacity text layer");
    let timestamp = RationalTime::zero(rate);
    let preview = preview_overlay_at(&half_opacity_project, &item_id, timestamp)
        .expect("half-opacity preview")
        .expect("active text layer");
    let full_frame = render_frame_overlay_buffer_at(
        &half_opacity_project,
        &opacity_plan,
        opacity_layer,
        timestamp,
    )
    .expect("half-opacity export buffer")
    .expect("shared text buffer");
    let preview_pixels =
        RgbaBuffer::from_png_bytes(&preview.roi_png_bytes).expect("decode half-opacity preview");
    let export_pixels = full_frame.crop(preview.roi);
    let mut compared_pixels = 0;
    for (preview_pixel, export_pixel) in preview_pixels
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .zip(export_pixels.data.as_chunks::<4>().0)
    {
        if preview_pixel[3] <= 16 {
            continue;
        }
        let expected_alpha = (preview_pixel[3] as f32 * 0.5).round() as i16;
        assert!(
            (export_pixel[3] as i16 - expected_alpha).abs() <= 2,
            "export alpha must apply keyframed 0.5 opacity to preview alpha"
        );
        compared_pixels += 1;
    }
    assert!(compared_pixels > 0, "compare visible text pixels");
}

#[test]
fn g53_keyframed_square_legacy_text_mp4_matches_preview_positions() {
    let (mut project, item_id) = project_with_rich_text("G53 MP4 legacy text", "Caption", None);
    project.settings.width = 320;
    project.settings.height = 180;
    project.settings.frame_rate = FrameRate::FPS_30;
    let rate = project.settings.frame_rate;
    let clip_range = TimeRange::new(RationalTime::zero(rate), RationalTime::from_frames(8, rate));
    let item = &mut project.tracks[0].items[0];
    item.timeline_range = clip_range;
    item.source_range = clip_range;
    item.text_style = Some(TextStyle {
        font_size: 34.0,
        background_color: Some("#E02020".to_string()),
        background_opacity: 1.0,
        background_corner_radius: 0.0,
        padding_x: 8.0,
        padding_y: 5.0,
        ..TextStyle::default()
    });
    item.transform.position_y = -100.0;
    item.keyframes = vec![
        Keyframe::new(KeyframeProperty::PositionY, RationalTime::zero(rate), 55.0),
        Keyframe::new(
            KeyframeProperty::PositionY,
            RationalTime::from_frames(7, rate),
            -40.0,
        ),
    ];
    project.validate().expect("G53 MP4 project is valid");

    let unique_dir =
        std::env::temp_dir().join(format!("tinixcut_g53_mp4_{}", uuid::Uuid::new_v4()));
    fs::create_dir(&unique_dir).expect("create unique G53 test directory");
    let output = unique_dir.join("g53_keyframed.mp4");
    let profile = ExportProfile::from_project_settings(&project.settings);
    let report = ExportEngine::render_snapshot(&project, &output, &profile, None)
        .expect("export keyframed square legacy caption");
    assert!(report.file_size_bytes > 0);

    let decoded = MediaBinaryResolver::command("ffmpeg")
        .expect("ffmpeg available")
        .args([
            "-v",
            "error",
            "-i",
            &output.to_string_lossy(),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])
        .output()
        .expect("decode exported video frames");
    assert!(decoded.status.success());
    let frame_size = (project.settings.width * project.settings.height * 4) as usize;
    assert!(
        decoded.stdout.len() >= frame_size * 8,
        "export has all 8 frames"
    );

    let red_box = |buffer: &RgbaBuffer, origin: [u32; 2]| -> Option<[u32; 4]> {
        let mut bounds = [u32::MAX, u32::MAX, 0, 0];
        let mut found = false;
        for y in 0..buffer.height {
            for x in 0..buffer.width {
                let offset = ((y as usize * buffer.width as usize) + x as usize) * 4;
                let [red, green, blue, alpha] = buffer.data[offset..offset + 4]
                    .try_into()
                    .expect("RGBA pixel");
                if alpha > 32
                    && red > 100
                    && red > green.saturating_mul(2)
                    && red > blue.saturating_mul(2)
                {
                    found = true;
                    let x = x + origin[0];
                    let y = y + origin[1];
                    bounds[0] = bounds[0].min(x);
                    bounds[1] = bounds[1].min(y);
                    bounds[2] = bounds[2].max(x);
                    bounds[3] = bounds[3].max(y);
                }
            }
        }
        found.then_some(bounds)
    };

    let mut preview_centers = Vec::new();
    let mut export_centers = Vec::new();
    for frame_index in [0_u32, 7_u32] {
        let timestamp = RationalTime::from_frames(frame_index as i64, rate);
        let preview = preview_overlay_at(&project, &item_id, timestamp)
            .expect("render preview overlay")
            .expect("caption active at keyframe");
        let preview_pixels =
            RgbaBuffer::from_png_bytes(&preview.roi_png_bytes).expect("decode preview ROI");
        let expected = red_box(&preview_pixels, [preview.roi[0], preview.roi[1]])
            .expect("preview contains red square background");

        let start = frame_index as usize * frame_size;
        let exported_frame = RgbaBuffer {
            width: project.settings.width,
            height: project.settings.height,
            data: decoded.stdout[start..start + frame_size].to_vec(),
        };
        let actual =
            red_box(&exported_frame, [0, 0]).expect("encoded frame contains red square background");
        for coordinate in 0..4 {
            assert!(
                expected[coordinate].abs_diff(actual[coordinate]) <= 3,
                "preview/export red-background bounds differ by more than codec tolerance at frame {frame_index}: expected {expected:?}, actual {actual:?}"
            );
        }
        preview_centers.push((expected[1] + expected[3]) / 2);
        export_centers.push((actual[1] + actual[3]) / 2);
    }
    assert!(preview_centers[0].abs_diff(preview_centers[1]) > 40);
    assert!(export_centers[0].abs_diff(export_centers[1]) > 40);

    let canonical_temp = std::env::temp_dir()
        .canonicalize()
        .expect("canonicalize temp directory");
    let canonical_test_dir = unique_dir
        .canonicalize()
        .expect("canonicalize created G53 test directory");
    assert_eq!(
        canonical_test_dir.parent(),
        Some(canonical_temp.as_path()),
        "test directory must be a direct child of the system temp directory"
    );
    let test_dir_name = canonical_test_dir
        .file_name()
        .and_then(|name| name.to_str())
        .expect("G53 test directory name");
    assert!(
        test_dir_name.starts_with("tinixcut_g53_mp4_"),
        "refuse to remove unexpected temp directory"
    );
    fs::remove_dir_all(&canonical_test_dir).expect("remove this test's unique temporary directory");
}

#[test]
fn p30_ac06_cache_invalidation_never_reuses_stale_masks() {
    let renderer = RichTextRenderer::new(1920, 1080, ResolvedFont::bundled());
    let style = OverlayStyle::from_rich(&base_rich("#FFFFFF"), None).expect("style");
    let first = renderer
        .render_overlay("Cache", &style, &OverlayPlacement::default())
        .expect("first render");
    let (entries, bytes) = renderer.cache_stats();
    assert!(entries > 0 && bytes > 0);

    renderer.clear_cache();
    assert_eq!(renderer.cache_stats(), (0, 0));
    let second = renderer
        .render_overlay("Cache", &style, &OverlayPlacement::default())
        .expect("second render");
    assert_eq!(first.roi_png_bytes, second.roi_png_bytes);

    // Style change must not reuse stale prepared masks (stroke bleed changes the ROI).
    let mut thicker = base_rich("#FFFFFF");
    thicker.strokes = vec![TextStroke {
        color: "#000000".to_string(),
        width: 12.0,
        align: StrokeAlign::Outside,
    }];
    let thick = render(&thicker, "Cache");
    assert!((thick.roi[2] - thick.roi[0]) > (first.roi[2] - first.roi[0]));
}

#[test]
fn p30_ac07_ac08_export_smoke_cleans_derived_artifacts() {
    let dir = test_dir("export_smoke");
    let (mut project, _item_id) =
        project_with_rich_text("Export smoke", "Tinix", Some(base_rich("#FFE35F")));
    project.settings.width = 320;
    project.settings.height = 180;
    project.settings.frame_rate = FrameRate::FPS_30;
    for track in &mut project.tracks {
        for item in &mut track.items {
            item.timeline_range = TimeRange::new(
                RationalTime::zero(FrameRate::FPS_30),
                RationalTime::from_frames(6, FrameRate::FPS_30),
            );
            item.source_range = item.timeline_range;
        }
    }
    let profile = ExportProfile::from_project_settings(&project.settings);
    let output = dir.join("rich_export.mp4");
    let report =
        ExportEngine::render_snapshot(&project, &output, &profile, None).expect("headless export");
    assert!(report.file_size_bytes > 0);
    assert!(output.exists());

    let leftovers: Vec<_> = fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("tc_text_"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "materialized text overlays must be removed: {leftovers:?}"
    );
    let overlay_dirs: Vec<_> = fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false)
                && entry.path().join("text_overlays").exists()
        })
        .collect();
    assert!(
        overlay_dirs.is_empty(),
        "text_overlays derived directory must be removed: {overlay_dirs:?}"
    );
    let automation_dirs: Vec<_> = fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("tc_auto_"))
        .collect();
    assert!(
        automation_dirs.is_empty(),
        "disposable automation directory must be removed: {automation_dirs:?}"
    );
    let lingering_pngs: Vec<_> = fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|extension| extension.eq_ignore_ascii_case("png"))
                .unwrap_or(false)
        })
        .collect();
    assert!(
        lingering_pngs.is_empty(),
        "derived overlay PNGs must not survive export: {lingering_pngs:?}"
    );

    // Decode one frame and prove glyph pixels reached the encoded video.
    let mut command = MediaBinaryResolver::command("ffmpeg").expect("ffmpeg available");
    let frame_bytes = command
        .args([
            "-v",
            "error",
            "-i",
            &output.to_string_lossy(),
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])
        .output()
        .expect("decode frame");
    assert!(frame_bytes.status.success());
    let decoded = RgbaBuffer {
        width: 320,
        height: 180,
        data: frame_bytes.stdout,
    };
    let visible = decoded
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[0] as u32 + pixel[1] as u32 + pixel[2] as u32 > 45)
        .count();
    assert!(
        visible > 0,
        "exported frame must contain rendered text pixels"
    );
}

#[test]
fn p30_ac08_missing_font_fails_export_instead_of_substituting() {
    let dir = test_dir("missing_font_export");
    let mut rich = base_rich("#FFFFFF");
    rich.font = Some(FontReference {
        family: "Unbundled Family".to_string(),
        weight: 400,
        style: Default::default(),
        asset_id: None,
    });
    let (project, _item_id) = project_with_rich_text("Missing font", "Tinix", Some(rich));
    let profile = ExportProfile::from_project_settings(&project.settings);
    let error = ExportEngine::render_snapshot(&project, &dir.join("out.mp4"), &profile, None)
        .expect_err("unbundled font must fail explicitly");
    let message = error.to_string();
    assert!(
        message.contains("Unbundled Family") && message.contains("unavailable"),
        "unexpected export error: {message}"
    );
}

#[test]
fn p30_review_descent_and_bleed_do_not_clip_diacritics_or_shadows() {
    let mut rich = base_rich("#FFFFFF");
    rich.strokes = vec![TextStroke {
        color: "#AD5100".to_string(),
        width: 7.0,
        align: StrokeAlign::Outside,
    }];
    rich.shadows = vec![
        TextShadow {
            color: "#AD5100E6".to_string(),
            offset_x: 8.0,
            offset_y: 8.0,
            blur: 0.0,
        },
        TextShadow {
            color: "#FFA758E6".to_string(),
            offset_x: 12.0,
            offset_y: 12.0,
            blur: 3.0,
        },
    ];
    let renderer = RichTextRenderer::new(1920, 1080, ResolvedFont::bundled());
    let style = OverlayStyle::from_rich(&rich, None).expect("style");
    let overlay = renderer
        .render_overlay("Tiếng Việt", &style, &OverlayPlacement::default())
        .expect("render");
    let roi_height = overlay.roi[3] - overlay.roi[1];
    assert!(
        roi_height >= 80,
        "descenders/underdots/shadows must be retained (roi height {roi_height})"
    );

    let full = renderer
        .render_overlay_buffer("Tiếng Việt", &style, &OverlayPlacement::default())
        .expect("buffer");
    let border_has_ink = (0..full.width).any(|x| {
        let top = ((x * 4) + 3) as usize;
        let bottom = (((full.height - 1) * full.width + x) * 4 + 3) as usize;
        full.data[top] > 0 || full.data[bottom] > 0
    }) || (0..full.height).any(|y| {
        let left = (y * full.width * 4 + 3) as usize;
        let right = ((y * full.width + full.width - 1) * 4 + 3) as usize;
        full.data[left] > 0 || full.data[right] > 0
    });
    assert!(
        !border_has_ink,
        "bleed must leave a transparent safety border"
    );
}

#[test]
fn p30_review_cache_signature_never_aliases_strokes_and_shadows() {
    let renderer = RichTextRenderer::new(1920, 1080, ResolvedFont::bundled());
    let mut stroke_style = base_rich("#FFFFFF");
    stroke_style.strokes = vec![TextStroke {
        color: "#FF0000".to_string(),
        width: 3.0,
        align: StrokeAlign::Outside,
    }];
    let mut shadow_style = base_rich("#FFFFFF");
    shadow_style.shadows = vec![TextShadow {
        color: "#0000FF".to_string(),
        offset_x: 0.0,
        offset_y: 0.0,
        blur: 3.0,
    }];
    let stroke = OverlayStyle::from_rich(&stroke_style, None).expect("style");
    let shadow = OverlayStyle::from_rich(&shadow_style, None).expect("style");

    let stroke_a = renderer
        .render_overlay("A", &stroke, &OverlayPlacement::default())
        .expect("stroke first");
    let shadow_a = renderer
        .render_overlay("A", &shadow, &OverlayPlacement::default())
        .expect("shadow second");
    assert_ne!(stroke_a.roi_png_bytes, shadow_a.roi_png_bytes);

    let fresh = RichTextRenderer::new(1920, 1080, ResolvedFont::bundled());
    fresh
        .render_overlay("A", &shadow, &OverlayPlacement::default())
        .expect("shadow first");
    fresh
        .render_overlay("A", &stroke, &OverlayPlacement::default())
        .expect("stroke second");
}

#[test]
fn p30_review_per_span_font_fails_explicitly() {
    let mut rich = base_rich("#FFFFFF");
    rich.spans = vec![RichTextSpan {
        start: 0,
        end: 2,
        style: TextSpanStyle {
            font: Some(FontReference {
                family: "Unbundled Span Font".to_string(),
                weight: 400,
                style: Default::default(),
                asset_id: None,
            }),
            ..TextSpanStyle::default()
        },
    }];
    let (project, _item_id) = project_with_rich_text("Span font", "Tiếng", Some(rich));
    let renderer = RichTextRenderer::new(1920, 1080, ResolvedFont::bundled());
    let plan = CompositionPlan::from_project(&project).expect("plan");
    let layer = plan.visual_layers.first().expect("layer");
    let rich_style = layer.text_rich.as_ref().expect("rich");
    let style = OverlayStyle::from_rich(rich_style, None).expect("style");
    let error = renderer
        .render_overlay("Tiếng", &style, &OverlayPlacement::default())
        .expect_err("per-span font must not be silently substituted");
    assert!(matches!(
        error,
        TextOverlayError::UnsupportedPerSpanFont { .. }
    ));

    let profile = ExportProfile::from_project_settings(&project.settings);
    let dir = test_dir("span_font_export");
    let export_error =
        ExportEngine::render_snapshot(&project, &dir.join("out.mp4"), &profile, None)
            .expect_err("export must fail explicitly");
    assert!(export_error.to_string().contains("Unbundled Span Font"));
}

/// Writes the Phase 30 evidence bundle. Ignored by default; run with
/// `TINIX_PHASE30_EVIDENCE_DIR=<repo>/artifacts/animated-text-templates/phase-30`
/// so CI does not mutate tracked artifacts.
#[test]
#[ignore = "writes evidence artifacts when TINIX_PHASE30_EVIDENCE_DIR is set"]
fn p30_write_evidence_artifacts() {
    let Some(dir) = std::env::var_os("TINIX_PHASE30_EVIDENCE_DIR").map(PathBuf::from) else {
        return;
    };
    fs::create_dir_all(&dir).expect("create evidence dir");

    let mut rich = base_rich("#FFFFFF");
    rich.fill = TextFill::Gradient {
        gradient_kind: GradientKind::Linear,
        angle_deg: 118.0,
        radius: None,
        space: GradientSpace::All,
        stops: vec![
            GradientStop {
                offset: 0.05,
                color: "#FFE35F".to_string(),
            },
            GradientStop {
                offset: 1.0,
                color: "#FFFFFF".to_string(),
            },
        ],
    };
    rich.strokes = vec![
        TextStroke {
            color: "#4A2000".to_string(),
            width: 7.0,
            align: StrokeAlign::Outside,
        },
        TextStroke {
            color: "#AD5100".to_string(),
            width: 4.0,
            align: StrokeAlign::Outside,
        },
    ];
    rich.shadows = vec![
        TextShadow {
            color: "#AD5100E6".to_string(),
            offset_x: 8.0,
            offset_y: 8.0,
            blur: 0.0,
        },
        TextShadow {
            color: "#FFA758E6".to_string(),
            offset_x: 12.0,
            offset_y: 12.0,
            blur: 3.0,
        },
    ];
    rich.layout = TextLayout {
        alignment: tinixcut_lib::modules::domain::TextAlignment::Center,
        line_height: 1.15,
        ..TextLayout::default()
    };

    let renderer = RichTextRenderer::new(1920, 1080, ResolvedFont::bundled());
    let style = OverlayStyle::from_rich(&rich, None).expect("style");
    let overlay = renderer
        .render_overlay("TINIX CapCut Sample", &style, &OverlayPlacement::default())
        .expect("render");
    write_evidence(&dir, "sample-rich-overlay.png", &overlay.roi_png_bytes);

    let full = renderer
        .render_overlay_buffer("TINIX CapCut Sample", &style, &OverlayPlacement::default())
        .expect("full buffer");
    for (name, background) in [
        ("alpha-white.png", ColorRgba::new(1.0, 1.0, 1.0, 1.0)),
        ("alpha-black.png", ColorRgba::new(0.0, 0.0, 0.0, 1.0)),
        ("alpha-color.png", ColorRgba::new(0.25, 0.45, 0.85, 1.0)),
    ] {
        let mut canvas = RgbaBuffer::new(full.width, full.height);
        canvas.fill_color(
            (background.r * 255.0).round() as u8,
            (background.g * 255.0).round() as u8,
            (background.b * 255.0).round() as u8,
            255,
        );
        canvas.blend_buffer_over(&full);
        write_evidence(&dir, name, &canvas.crop(overlay.roi).to_png_bytes());
    }

    // Preview and export packets must be byte-identical for the same canonical layer.
    let (project, item_id) =
        project_with_rich_text("Evidence parity", "Tiếng Việt", Some(rich.clone()));
    let timestamp = RationalTime::new(0, project.settings.frame_rate);
    let preview = preview_overlay_at(&project, &item_id, timestamp)
        .expect("preview")
        .expect("active");
    let plan = CompositionPlan::from_project(&project).expect("plan");
    let layer = plan
        .visual_layers
        .iter()
        .find(|candidate| candidate.item_id == item_id)
        .expect("layer");
    let exported =
        render_overlay_for_layer(&project, plan.width, plan.height, layer, layer.opacity)
            .expect("export")
            .expect("rich");
    assert_eq!(preview.roi_png_bytes, exported.roi_png_bytes);
    write_evidence(&dir, "parity-preview.png", &preview.roi_png_bytes);
    write_evidence(&dir, "parity-export.png", &exported.roi_png_bytes);

    // Small real export for visual/video evidence.
    let mut smoke = project.clone();
    smoke.settings.width = 640;
    smoke.settings.height = 360;
    for track in &mut smoke.tracks {
        for item in &mut track.items {
            item.timeline_range = TimeRange::new(
                RationalTime::zero(FrameRate::FPS_30),
                RationalTime::from_frames(15, FrameRate::FPS_30),
            );
            item.source_range = item.timeline_range;
        }
    }
    let profile = ExportProfile::from_project_settings(&smoke.settings);
    let evidence_video = dir.join("export-smoke.mp4");
    // Promote atomically; a stale evidence file must be absent before rerun.
    let _ = fs::remove_file(&evidence_video);
    let report = ExportEngine::render_snapshot(&smoke, &evidence_video, &profile, None)
        .expect("evidence export");
    let export_metrics = serde_json::json!({
        "file_size_bytes": report.file_size_bytes,
        "render_time_ms": report.render_time_ms,
        "duration_secs": report.duration_secs,
        "encoder_used": report.encoder_used,
        "overlay_roi": overlay.roi,
        "font_sha256": overlay.diagnostics.font_sha256,
        "missing_glyphs": overlay.diagnostics.missing_glyphs,
        "renderer_version": overlay.diagnostics.renderer_version,
    })
    .to_string();
    write_evidence(&dir, "export-metrics.json", export_metrics.as_bytes());

    // Workload W1/W2 on the production renderer (Phase 27 budgets).
    // 120 graphemes (Phase 27 W1 definition).
    let w1_text = "TINIX rich text parity sample ".repeat(4);
    let mut w1_style = style.clone();
    w1_style.max_width = Some(1600.0);
    w1_style.wrap_mode = TextWrapMode::Wrap;
    let cold_renderer = RichTextRenderer::new(1920, 1080, ResolvedFont::bundled());
    let cold_started = std::time::Instant::now();
    cold_renderer
        .render_overlay(&w1_text, &w1_style, &OverlayPlacement::default())
        .expect("cold W1");
    let cold_ms = cold_started.elapsed().as_secs_f64() * 1000.0;

    let mut samples = Vec::with_capacity(60);
    for _ in 0..60 {
        let started = std::time::Instant::now();
        cold_renderer
            .render_overlay_buffer(&w1_text, &w1_style, &OverlayPlacement::default())
            .expect("warm W1 buffer");
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let buffer_p50 = samples[samples.len() / 2];
    let buffer_p95 = percentile(&samples, 0.95);

    let mut packet_samples = Vec::with_capacity(60);
    for _ in 0..60 {
        let started = std::time::Instant::now();
        cold_renderer
            .render_overlay(&w1_text, &w1_style, &OverlayPlacement::default())
            .expect("warm W1 packet");
        packet_samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    packet_samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let packet_p95 = percentile(&packet_samples, 0.95);

    let titles = ["Title One", "Title Two", "Title Three", "Title Four"];
    let mut w2_samples = Vec::with_capacity(30);
    for _ in 0..30 {
        let started = std::time::Instant::now();
        for (index, title) in titles.iter().enumerate() {
            let placement = OverlayPlacement {
                offset_x: 0.0,
                offset_y: (index as f32 - 1.5) * 210.0,
                ..OverlayPlacement::default()
            };
            cold_renderer
                .render_overlay(title, &w1_style, &placement)
                .expect("W2 title");
        }
        w2_samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    w2_samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let w2_p95 = percentile(&w2_samples, 0.95);

    let mut sample_packet = Vec::with_capacity(60);
    for _ in 0..60 {
        let started = std::time::Instant::now();
        cold_renderer
            .render_overlay("TINIX CapCut Sample", &style, &OverlayPlacement::default())
            .expect("sample packet");
        sample_packet.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    sample_packet.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let sample_packet_p95 = percentile(&sample_packet, 0.95);

    let (cache_entries, cache_bytes) = cold_renderer.cache_stats();
    let benchmark_metrics = serde_json::json!({
        "profile": "CARGO_PROFILE_TEST_OPT_LEVEL=3",
        "canvas": [1920, 1080],
        "w1": {
            "cold_ms": cold_ms,
            "buffer_warm_p50_ms": buffer_p50,
            "buffer_warm_p95_ms": buffer_p95,
            "packet_warm_p95_ms": packet_p95,
            "budget_p95_ms": 16.0,
        },
        "w2": {
            "titles": 4,
            "packet_warm_p95_ms": w2_p95,
            "budget_p95_ms": 33.3,
        },
        "sample_one_line_packet_p95_ms": sample_packet_p95,
        "cache_entries": cache_entries,
        "cache_bytes": cache_bytes,
        "cache_ceiling_bytes": 256 * 1024 * 1024,
    })
    .to_string();
    write_evidence(
        &dir,
        "benchmark-production.json",
        benchmark_metrics.as_bytes(),
    );
}

fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    let index = ((sorted.len() as f64 * quantile).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len().saturating_sub(1));
    sorted[index]
}
