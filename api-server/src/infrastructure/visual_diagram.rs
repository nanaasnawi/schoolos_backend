use serde_json::{json, Value};
use sqlx::PgPool;
use std::f64::consts::PI;
use uuid::Uuid;

/// Escapes special XML characters so user/AI text never breaks SVG structure.
fn escape_xml(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 8);
    for c in raw.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn clip_label(raw: &str, max_chars: usize) -> String {
    let trimmed = raw.trim();
    let count = trimmed.chars().count();
    if count <= max_chars {
        trimmed.to_string()
    } else {
        let taken: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{taken}…")
    }
}

fn format_num(val: f64) -> String {
    if (val - val.round()).abs() < 1e-6 {
        format!("{}", val.round() as i64)
    } else {
        let s = format!("{val:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn svg_frame(badge: &str, title: &str, subtitle: Option<&str>, inner: &str) -> String {
    let clean_badge = escape_xml(&clip_label(badge, 32));
    let clean_title = escape_xml(&clip_label(title, 68));
    let sub_el = subtitle
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            format!(
                r##"<text x="400" y="74" text-anchor="middle" font-size="12.5" font-weight="500" fill="#64748B">{}</text>"##,
                escape_xml(&clip_label(s, 84))
            )
        })
        .unwrap_or_default();

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 440" width="800" height="440" font-family="system-ui, -apple-system, 'Segoe UI', Roboto, Arial, sans-serif">
  <defs>
    <marker id="arrow" viewBox="0 0 10 10" refX="8.5" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 1.5 L 10 5 L 0 8.5 z" fill="#475569"/>
    </marker>
    <marker id="arrow-emerald" viewBox="0 0 10 10" refX="8.5" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 1.5 L 10 5 L 0 8.5 z" fill="#059669"/>
    </marker>
    <marker id="arrow-blue" viewBox="0 0 10 10" refX="8.5" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 1.5 L 10 5 L 0 8.5 z" fill="#0284C7"/>
    </marker>
  </defs>
  <rect x="0" y="0" width="800" height="440" rx="16" fill="#FFFFFF"/>
  <rect x="6" y="6" width="788" height="428" rx="12" fill="#F8FAFC" stroke="#CBD5E1" stroke-width="1.5"/>
  <rect x="24" y="18" width="144" height="22" rx="11" fill="#E0F2FE" stroke="#BAE6FD" stroke-width="1"/>
  <text x="96" y="33" text-anchor="middle" font-size="10.5" font-weight="700" fill="#0369A1" letter-spacing="0.6">{clean_badge}</text>
  <text x="400" y="54" text-anchor="middle" font-size="18" font-weight="800" fill="#0F172A">{clean_title}</text>
  {sub_el}
  {inner}
</svg>"##
    )
}

const PALETTE: [&str; 6] = [
    "#0284C7", // Sky
    "#059669", // Emerald
    "#D97706", // Amber
    "#7C3AED", // Violet
    "#E11D48", // Rose
    "#0D9488", // Teal
];

fn extract_chart_points(spec: &Value, keys: &[&str]) -> Vec<(String, f64)> {
    for k in keys {
        if let Some(arr) = spec.get(*k).and_then(|v| v.as_array()) {
            let parsed: Vec<(String, f64)> = arr
                .iter()
                .enumerate()
                .filter_map(|(i, item)| {
                    let lbl = item
                        .get("label")
                        .or_else(|| item.get("name"))
                        .or_else(|| item.get("x"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| format!("Data {}", i + 1));
                    let val = item
                        .get("value")
                        .or_else(|| item.get("y"))
                        .or_else(|| item.get("count"))
                        .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse::<f64>().ok())))?;
                    Some((lbl, val.max(0.0)))
                })
                .take(8)
                .collect();
            if !parsed.is_empty() {
                return parsed;
            }
        }
    }
    vec![
        ("Kelompok A".to_string(), 25.0),
        ("Kelompok B".to_string(), 40.0),
        ("Kelompok C".to_string(), 30.0),
        ("Kelompok D".to_string(), 45.0),
    ]
}

fn render_bar_chart(spec: &Value) -> String {
    let title = spec.get("title").and_then(|v| v.as_str()).unwrap_or("Grafik Batang Data");
    let x_label = spec.get("x_label").and_then(|v| v.as_str()).unwrap_or("");
    let y_label = spec.get("y_label").and_then(|v| v.as_str()).unwrap_or("");
    let unit = spec.get("unit").and_then(|v| v.as_str()).unwrap_or("");
    let bars = extract_chart_points(spec, &["bars", "data", "items"]);

    let raw_max = bars.iter().map(|(_, v)| *v).fold(10.0_f64, f64::max);
    let y_max = if raw_max <= 0.0 { 10.0 } else { (raw_max * 1.18).ceil() };

    let plot_x = 96.0;
    let plot_y = 100.0;
    let plot_w = 640.0;
    let plot_h = 250.0;

    let mut inner = String::new();
    // Gridlines
    for step in 0..=4 {
        let frac = step as f64 / 4.0;
        let gy = plot_y + plot_h - frac * plot_h;
        let tick_val = y_max * frac;
        inner.push_str(&format!(
            r##"<line x1="{plot_x}" y1="{gy:.1}" x2="{x_end}" y2="{gy:.1}" stroke="#E2E8F0" stroke-width="1" stroke-dasharray="4 3"/>"##,
            x_end = plot_x + plot_w
        ));
        inner.push_str(&format!(
            r##"<text x="{lx:.1}" y="{ly:.1}" text-anchor="end" font-size="11.5" font-weight="600" fill="#64748B">{val}</text>"##,
            lx = plot_x - 10.0,
            ly = gy + 4.0,
            val = format_num(tick_val)
        ));
    }

    // Axes
    inner.push_str(&format!(
        r##"<line x1="{plot_x}" y1="{plot_y}" x2="{plot_x}" y2="{y_bot}" stroke="#334155" stroke-width="2"/>
            <line x1="{plot_x}" y1="{y_bot}" x2="{x_end}" y2="{y_bot}" stroke="#334155" stroke-width="2"/>"##,
        y_bot = plot_y + plot_h,
        x_end = plot_x + plot_w
    ));

    if !y_label.trim().is_empty() {
        inner.push_str(&format!(
            r##"<text x="28" y="{cy:.1}" text-anchor="middle" transform="rotate(-90 28 {cy:.1})" font-size="12" font-weight="700" fill="#475569">{lbl}</text>"##,
            cy = plot_y + plot_h / 2.0,
            lbl = escape_xml(&clip_label(y_label, 28))
        ));
    }
    if !x_label.trim().is_empty() {
        inner.push_str(&format!(
            r##"<text x="{cx:.1}" y="416" text-anchor="middle" font-size="12.5" font-weight="700" fill="#475569">{lbl}</text>"##,
            cx = plot_x + plot_w / 2.0,
            lbl = escape_xml(&clip_label(x_label, 48))
        ));
    }

    let slot_w = plot_w / bars.len().max(1) as f64;
    let bar_w = (slot_w * 0.56).clamp(28.0, 88.0);
    for (idx, (lbl, val)) in bars.iter().enumerate() {
        let cx = plot_x + slot_w * (idx as f64 + 0.5);
        let bx = cx - bar_w / 2.0;
        let bh = ((val / y_max) * plot_h).max(2.0);
        let by = plot_y + plot_h - bh;
        let color = PALETTE[idx % PALETTE.len()];
        let val_text = if unit.trim().is_empty() {
            format_num(*val)
        } else {
            format!("{} {}", format_num(*val), unit.trim())
        };

        inner.push_str(&format!(
            r##"<rect x="{bx:.1}" y="{by:.1}" width="{bar_w:.1}" height="{bh:.1}" rx="6" fill="{color}"/>
               <text x="{cx:.1}" y="{vy:.1}" text-anchor="middle" font-size="12.5" font-weight="800" fill="#0F172A">{val_lbl}</text>
               <text x="{cx:.1}" y="{xy:.1}" text-anchor="middle" font-size="12" font-weight="600" fill="#334155">{x_lbl}</text>"##,
            vy = (by - 8.0).max(plot_y - 4.0),
            val_lbl = escape_xml(&val_text),
            xy = plot_y + plot_h + 22.0,
            x_lbl = escape_xml(&clip_label(lbl, 16))
        ));
    }

    svg_frame("GRAFIK BATANG", title, None, &inner)
}

fn render_line_chart(spec: &Value) -> String {
    let title = spec.get("title").and_then(|v| v.as_str()).unwrap_or("Grafik Garis Perkembangan");
    let x_label = spec.get("x_label").and_then(|v| v.as_str()).unwrap_or("");
    let y_label = spec.get("y_label").and_then(|v| v.as_str()).unwrap_or("");
    let unit = spec.get("unit").and_then(|v| v.as_str()).unwrap_or("");
    let pts = extract_chart_points(spec, &["points", "data", "bars"]);

    let raw_max = pts.iter().map(|(_, v)| *v).fold(10.0_f64, f64::max);
    let y_max = if raw_max <= 0.0 { 10.0 } else { (raw_max * 1.18).ceil() };
    let plot_x = 96.0;
    let plot_y = 100.0;
    let plot_w = 640.0;
    let plot_h = 250.0;

    let mut inner = String::new();
    for step in 0..=4 {
        let frac = step as f64 / 4.0;
        let gy = plot_y + plot_h - frac * plot_h;
        let tick_val = y_max * frac;
        inner.push_str(&format!(
            r##"<line x1="{plot_x}" y1="{gy:.1}" x2="{x_end}" y2="{gy:.1}" stroke="#E2E8F0" stroke-width="1" stroke-dasharray="4 3"/>
               <text x="{lx:.1}" y="{ly:.1}" text-anchor="end" font-size="11.5" font-weight="600" fill="#64748B">{val}</text>"##,
            x_end = plot_x + plot_w,
            lx = plot_x - 10.0,
            ly = gy + 4.0,
            val = format_num(tick_val)
        ));
    }

    inner.push_str(&format!(
        r##"<line x1="{plot_x}" y1="{plot_y}" x2="{plot_x}" y2="{y_bot}" stroke="#334155" stroke-width="2"/>
            <line x1="{plot_x}" y1="{y_bot}" x2="{x_end}" y2="{y_bot}" stroke="#334155" stroke-width="2"/>"##,
        y_bot = plot_y + plot_h,
        x_end = plot_x + plot_w
    ));

    if !y_label.trim().is_empty() {
        inner.push_str(&format!(
            r##"<text x="28" y="{cy:.1}" text-anchor="middle" transform="rotate(-90 28 {cy:.1})" font-size="12" font-weight="700" fill="#475569">{lbl}</text>"##,
            cy = plot_y + plot_h / 2.0,
            lbl = escape_xml(&clip_label(y_label, 28))
        ));
    }
    if !x_label.trim().is_empty() {
        inner.push_str(&format!(
            r##"<text x="{cx:.1}" y="416" text-anchor="middle" font-size="12.5" font-weight="700" fill="#475569">{lbl}</text>"##,
            cx = plot_x + plot_w / 2.0,
            lbl = escape_xml(&clip_label(x_label, 48))
        ));
    }

    let n = pts.len().max(1);
    let mut coords: Vec<(f64, f64, &str, f64)> = Vec::with_capacity(n);
    for (i, (lbl, val)) in pts.iter().enumerate() {
        let px = if n == 1 {
            plot_x + plot_w / 2.0
        } else {
            plot_x + 42.0 + (i as f64 / (n - 1) as f64) * (plot_w - 84.0)
        };
        let py = plot_y + plot_h - (val / y_max) * plot_h;
        coords.push((px, py, lbl.as_str(), *val));
    }

    if coords.len() >= 2 {
        let poly_str: String = coords
            .iter()
            .map(|(x, y, _, _)| format!("{x:.1},{y:.1}"))
            .collect::<Vec<_>>()
            .join(" ");
        inner.push_str(&format!(
            r##"<polyline fill="none" stroke="#0284C7" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round" points="{poly_str}"/>"##
        ));
    }

    for (px, py, lbl, val) in coords {
        let val_text = if unit.trim().is_empty() {
            format_num(val)
        } else {
            format!("{} {}", format_num(val), unit.trim())
        };
        inner.push_str(&format!(
            r##"<circle cx="{px:.1}" cy="{py:.1}" r="6" fill="#0284C7" stroke="#FFFFFF" stroke-width="2.5"/>
               <rect x="{rx:.1}" y="{ry:.1}" width="58" height="20" rx="5" fill="#FFFFFF" stroke="#CBD5E1" stroke-width="1"/>
               <text x="{px:.1}" y="{vy:.1}" text-anchor="middle" font-size="11.5" font-weight="800" fill="#0F172A">{val_lbl}</text>
               <text x="{px:.1}" y="{xy:.1}" text-anchor="middle" font-size="11.5" font-weight="600" fill="#334155">{x_lbl}</text>"##,
            rx = px - 29.0,
            ry = py - 29.0,
            vy = py - 15.0,
            val_lbl = escape_xml(&clip_label(&val_text, 10)),
            xy = plot_y + plot_h + 22.0,
            x_lbl = escape_xml(&clip_label(lbl, 15))
        ));
    }

    svg_frame("GRAFIK GARIS", title, None, &inner)
}

fn render_pie_chart(spec: &Value) -> String {
    let title = spec.get("title").and_then(|v| v.as_str()).unwrap_or("Diagram Lingkaran");
    let slices = extract_chart_points(spec, &["slices", "data", "bars"]);
    let total: f64 = slices.iter().map(|(_, v)| *v).sum();
    let total_safe = if total <= 0.0 { 1.0 } else { total };

    let cx = 240.0;
    let cy = 248.0;
    let r = 132.0;

    let mut inner = String::new();
    let mut start_angle = -PI / 2.0;

    for (idx, (lbl, val)) in slices.iter().enumerate() {
        let frac = *val / total_safe;
        let pct = (frac * 100.0).round();
        let color = PALETTE[idx % PALETTE.len()];

        if slices.len() == 1 || frac >= 0.999 {
            inner.push_str(&format!(
                r##"<circle cx="{cx}" cy="{cy}" r="{r}" fill="{color}" stroke="#FFFFFF" stroke-width="2"/>"##
            ));
        } else {
            let sweep = frac * 2.0 * PI;
            let end_angle = start_angle + sweep;
            let x1 = cx + r * start_angle.cos();
            let y1 = cy + r * start_angle.sin();
            let x2 = cx + r * end_angle.cos();
            let y2 = cy + r * end_angle.sin();
            let large_arc = if sweep > PI { 1 } else { 0 };
            inner.push_str(&format!(
                r##"<path d="M {cx:.1} {cy:.1} L {x1:.1} {y1:.1} A {r} {r} 0 {large_arc} 1 {x2:.1} {y2:.1} Z" fill="{color}" stroke="#FFFFFF" stroke-width="2.5"/>"##
            ));
            let mid = start_angle + sweep / 2.0;
            if frac >= 0.07 {
                let tx = cx + (r * 0.64) * mid.cos();
                let ty = cy + (r * 0.64) * mid.sin() + 4.0;
                inner.push_str(&format!(
                    r##"<text x="{tx:.1}" y="{ty:.1}" text-anchor="middle" font-size="13" font-weight="800" fill="#FFFFFF">{pct:.0}%</text>"##
                ));
            }
            start_angle = end_angle;
        }

        // Right-hand legend item
        let ly = 128.0 + idx as f64 * 42.0;
        if ly <= 395.0 {
            inner.push_str(&format!(
                r##"<rect x="435" y="{ly:.1}" width="18" height="18" rx="4" fill="{color}"/>
                   <text x="463" y="{ty:.1}" font-size="13.5" font-weight="700" fill="#1E293B">{lbl} — {val_s} ({pct:.0}%)</text>"##,
                ty = ly + 14.0,
                lbl = escape_xml(&clip_label(lbl, 22)),
                val_s = format_num(*val)
            ));
        }
    }

    svg_frame("DIAGRAM LINGKARAN", title, None, &inner)
}

fn dim_str(dims: &Value, keys: &[&str], fallback: &str) -> String {
    for k in keys {
        if let Some(v) = dims.get(*k) {
            if let Some(s) = v.as_str() {
                if !s.trim().is_empty() {
                    return s.trim().to_string();
                }
            } else if let Some(n) = v.as_f64() {
                return format_num(n);
            }
        }
    }
    fallback.to_string()
}

fn vertex_label(labels: &[String], idx: usize, default_lbl: &str) -> String {
    labels
        .get(idx)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(default_lbl)
        .to_string()
}

fn render_geometry(spec: &Value) -> String {
    let title = spec.get("title").and_then(|v| v.as_str()).unwrap_or("Bangun Geometri");
    let shape = spec
        .get("shape")
        .and_then(|v| v.as_str())
        .unwrap_or("right_triangle")
        .trim()
        .to_lowercase();
    let dims = spec.get("dimensions").unwrap_or(&Value::Null);
    let labels: Vec<String> = spec
        .get("labels")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                .collect()
        })
        .unwrap_or_default();

    let v0 = escape_xml(&vertex_label(&labels, 0, "A"));
    let v1 = escape_xml(&vertex_label(&labels, 1, "B"));
    let v2 = escape_xml(&vertex_label(&labels, 2, "C"));
    let v3 = escape_xml(&vertex_label(&labels, 3, "D"));

    let inner = if shape.contains("rectangle") || shape.contains("persegi_panjang") {
        let p = escape_xml(&dim_str(dims, &["p", "panjang", "width", "a", "length"], "12 cm"));
        let l = escape_xml(&dim_str(dims, &["l", "lebar", "height", "b", "width_y"], "8 cm"));
        format!(
            r##"<rect x="210" y="135" width="380" height="190" fill="#E0F2FE" fill-opacity="0.55" stroke="#0284C7" stroke-width="3.5" rx="3"/>
               <polyline points="210,305 230,305 230,325" fill="none" stroke="#0369A1" stroke-width="2"/>
               <text x="195" y="128" text-anchor="end" font-size="16" font-weight="800" fill="#0F172A">{v0}</text>
               <text x="605" y="128" text-anchor="start" font-size="16" font-weight="800" fill="#0F172A">{v1}</text>
               <text x="605" y="342" text-anchor="start" font-size="16" font-weight="800" fill="#0F172A">{v2}</text>
               <text x="195" y="342" text-anchor="end" font-size="16" font-weight="800" fill="#0F172A">{v3}</text>
               <text x="400" y="362" text-anchor="middle" font-size="15.5" font-weight="800" fill="#0369A1">panjang = {p}</text>
               <text x="620" y="236" text-anchor="start" font-size="15.5" font-weight="800" fill="#059669">lebar = {l}</text>"##
        )
    } else if shape.contains("square") || shape == "persegi" {
        let s = escape_xml(&dim_str(dims, &["s", "sisi", "side", "a"], "10 cm"));
        format!(
            r##"<rect x="290" y="125" width="220" height="220" fill="#D1FAE5" fill-opacity="0.6" stroke="#059669" stroke-width="3.5" rx="3"/>
               <polyline points="290,325 310,325 310,345" fill="none" stroke="#047857" stroke-width="2"/>
               <text x="275" y="118" text-anchor="end" font-size="16" font-weight="800" fill="#0F172A">{v0}</text>
               <text x="525" y="118" text-anchor="start" font-size="16" font-weight="800" fill="#0F172A">{v1}</text>
               <text x="525" y="362" text-anchor="start" font-size="16" font-weight="800" fill="#0F172A">{v2}</text>
               <text x="275" y="362" text-anchor="end" font-size="16" font-weight="800" fill="#0F172A">{v3}</text>
               <text x="400" y="380" text-anchor="middle" font-size="15.5" font-weight="800" fill="#047857">s = {s}</text>"##
        )
    } else if shape.contains("circle") || shape.contains("lingkaran") {
        let has_d = dims.get("d").or_else(|| dims.get("diameter")).is_some()
            && dims.get("r").or_else(|| dims.get("radius")).is_none();
        let val = escape_xml(&dim_str(dims, &["r", "radius", "jari", "d", "diameter"], "14 cm"));
        let line = if has_d {
            format!(
                r##"<line x1="275" y1="245" x2="525" y2="245" stroke="#0284C7" stroke-width="3" stroke-dasharray="6 4"/>
                   <text x="400" y="230" text-anchor="middle" font-size="16" font-weight="800" fill="#0369A1">d = {val}</text>"##
            )
        } else {
            format!(
                r##"<line x1="400" y1="245" x2="525" y2="245" stroke="#0284C7" stroke-width="3"/>
                   <text x="462" y="232" text-anchor="middle" font-size="16" font-weight="800" fill="#0369A1">r = {val}</text>"##
            )
        };
        format!(
            r##"<circle cx="400" cy="245" r="125" fill="#E0F2FE" fill-opacity="0.45" stroke="#0284C7" stroke-width="3.5"/>
               <circle cx="400" cy="245" r="5" fill="#0F172A"/>
               <text x="392" y="268" text-anchor="end" font-size="15" font-weight="800" fill="#0F172A">O</text>
               {line}"##
        )
    } else if shape.contains("trapezoid") || shape.contains("trapesium") {
        let top = escape_xml(&dim_str(dims, &["a", "top", "atas"], "10 cm"));
        let bot = escape_xml(&dim_str(dims, &["b", "bottom", "bawah"], "18 cm"));
        let h = escape_xml(&dim_str(dims, &["t", "h", "height", "tinggi"], "8 cm"));
        format!(
            r##"<polygon points="280,145 520,145 600,335 200,335" fill="#EDE9FE" fill-opacity="0.6" stroke="#7C3AED" stroke-width="3.5"/>
               <line x1="280" y1="145" x2="280" y2="335" stroke="#D97706" stroke-width="2.5" stroke-dasharray="6 4"/>
               <polyline points="280,317 298,317 298,335" fill="none" stroke="#D97706" stroke-width="2"/>
               <text x="270" y="136" text-anchor="end" font-size="16" font-weight="800" fill="#0F172A">{v0}</text>
               <text x="530" y="136" text-anchor="start" font-size="16" font-weight="800" fill="#0F172A">{v1}</text>
               <text x="612" y="346" text-anchor="start" font-size="16" font-weight="800" fill="#0F172A">{v2}</text>
               <text x="188" y="346" text-anchor="end" font-size="16" font-weight="800" fill="#0F172A">{v3}</text>
               <text x="400" y="132" text-anchor="middle" font-size="15" font-weight="800" fill="#5B21B6">{top}</text>
               <text x="400" y="365" text-anchor="middle" font-size="15" font-weight="800" fill="#5B21B6">{bot}</text>
               <text x="292" y="246" text-anchor="start" font-size="14.5" font-weight="800" fill="#B45309">t = {h}</text>"##
        )
    } else if shape.contains("cube") || shape.contains("cuboid") || shape.contains("kubus") || shape.contains("balok") {
        let is_cube = shape.contains("cube") || shape.contains("kubus");
        let p = escape_xml(&dim_str(dims, &["p", "s", "sisi", "panjang"], if is_cube { "8 cm" } else { "12 cm" }));
        let l = escape_xml(&dim_str(dims, &["l", "lebar", "width"], &p));
        let t = escape_xml(&dim_str(dims, &["t", "tinggi", "height"], &p));
        format!(
            r##"<polygon points="270,185 470,185 470,335 270,335" fill="#E0F2FE" fill-opacity="0.55" stroke="#0284C7" stroke-width="3"/>
               <polygon points="270,185 350,125 550,125 470,185" fill="#BAE6FD" fill-opacity="0.55" stroke="#0284C7" stroke-width="3"/>
               <polygon points="470,185 550,125 550,275 470,335" fill="#7DD3FC" fill-opacity="0.55" stroke="#0284C7" stroke-width="3"/>
               <line x1="350" y1="125" x2="350" y2="275" stroke="#0369A1" stroke-width="2" stroke-dasharray="5 4"/>
               <line x1="350" y1="275" x2="270" y2="335" stroke="#0369A1" stroke-width="2" stroke-dasharray="5 4"/>
               <line x1="350" y1="275" x2="550" y2="275" stroke="#0369A1" stroke-width="2" stroke-dasharray="5 4"/>
               <text x="370" y="362" text-anchor="middle" font-size="15.5" font-weight="800" fill="#0369A1">{p}</text>
               <text x="530" y="318" text-anchor="start" font-size="15" font-weight="800" fill="#059669">{l}</text>
               <text x="565" y="205" text-anchor="start" font-size="15" font-weight="800" fill="#B45309">{t}</text>"##
        )
    } else if shape.contains("angle") || shape.contains("sudut") {
        let deg = escape_xml(&dim_str(dims, &["angle_deg", "deg", "sudut"], "60°"));
        format!(
            r##"<line x1="250" y1="315" x2="565" y2="315" stroke="#0F172A" stroke-width="3.5" marker-end="url(#arrow)"/>
               <line x1="250" y1="315" x2="465" y2="145" stroke="#0284C7" stroke-width="3.5" marker-end="url(#arrow-blue)"/>
               <path d="M 340 315 A 90 90 0 0 0 318 258" fill="none" stroke="#D97706" stroke-width="3"/>
               <circle cx="250" cy="315" r="5" fill="#0F172A"/>
               <text x="234" y="330" text-anchor="end" font-size="16" font-weight="800" fill="#0F172A">{v1}</text>
               <text x="565" y="340" text-anchor="middle" font-size="16" font-weight="800" fill="#0F172A">{v2}</text>
               <text x="475" y="138" text-anchor="start" font-size="16" font-weight="800" fill="#0F172A">{v0}</text>
               <text x="360" y="286" font-size="17" font-weight="800" fill="#B45309">{deg}</text>"##
        )
    } else if shape == "triangle" || shape == "segitiga" {
        let base = escape_xml(&dim_str(dims, &["a", "alas", "base"], "12 cm"));
        let height = escape_xml(&dim_str(dims, &["t", "h", "height", "tinggi"], "8 cm"));
        format!(
            r##"<polygon points="400,130 570,340 230,340" fill="#D1FAE5" fill-opacity="0.55" stroke="#059669" stroke-width="3.5"/>
               <line x1="400" y1="130" x2="400" y2="340" stroke="#D97706" stroke-width="2.5" stroke-dasharray="6 4"/>
               <polyline points="400,320 420,320 420,340" fill="none" stroke="#D97706" stroke-width="2"/>
               <text x="400" y="118" text-anchor="middle" font-size="16" font-weight="800" fill="#0F172A">{v0}</text>
               <text x="216" y="352" text-anchor="end" font-size="16" font-weight="800" fill="#0F172A">{v1}</text>
               <text x="584" y="352" text-anchor="start" font-size="16" font-weight="800" fill="#0F172A">{v2}</text>
               <text x="400" y="368" text-anchor="middle" font-size="15.5" font-weight="800" fill="#047857">alas = {base}</text>
               <text x="412" y="255" text-anchor="start" font-size="15" font-weight="800" fill="#B45309">t = {height}</text>"##
        )
    } else {
        // Default: right triangle (segitiga siku-siku)
        let a = escape_xml(&dim_str(dims, &["a", "alas", "base"], "6 cm"));
        let b = escape_xml(&dim_str(dims, &["b", "tinggi", "height", "t"], "8 cm"));
        let c = escape_xml(&dim_str(dims, &["c", "hypotenuse", "miring"], "? cm"));
        format!(
            r##"<polygon points="240,135 240,345 550,345" fill="#E0F2FE" fill-opacity="0.55" stroke="#0284C7" stroke-width="3.5"/>
               <polyline points="240,321 264,321 264,345" fill="none" stroke="#0369A1" stroke-width="2.5"/>
               <text x="228" y="130" text-anchor="end" font-size="16" font-weight="800" fill="#0F172A">{v0}</text>
               <text x="226" y="356" text-anchor="end" font-size="16" font-weight="800" fill="#0F172A">{v1}</text>
               <text x="564" y="356" text-anchor="start" font-size="16" font-weight="800" fill="#0F172A">{v2}</text>
               <text x="220" y="246" text-anchor="end" font-size="15.5" font-weight="800" fill="#059669">{b}</text>
               <text x="395" y="372" text-anchor="middle" font-size="15.5" font-weight="800" fill="#0369A1">{a}</text>
               <text x="415" y="226" text-anchor="start" font-size="15.5" font-weight="800" fill="#B45309">{c}</text>"##
        )
    };

    svg_frame("BANGUN GEOMETRI", title, None, &inner)
}

fn render_flowchart(spec: &Value) -> String {
    let title = spec.get("title").and_then(|v| v.as_str()).unwrap_or("Bagan Alur Konsep");
    let layout = spec.get("layout").and_then(|v| v.as_str()).unwrap_or("linear").to_lowercase();
    let steps: Vec<(String, String)> = spec
        .get("steps")
        .or_else(|| spec.get("items"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    if let Some(txt) = s.as_str() {
                        Some((txt.trim().to_string(), String::new()))
                    } else {
                        let lbl = s
                            .get("label")
                            .or_else(|| s.get("title"))
                            .or_else(|| s.get("text"))
                            .and_then(|v| v.as_str())
                            .map(|x| x.trim().to_string())
                            .filter(|x| !x.is_empty())
                            .unwrap_or_else(|| format!("Tahap {}", i + 1));
                        let note = s
                            .get("note")
                            .or_else(|| s.get("sublabel"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        Some((lbl, note))
                    }
                })
                .take(5)
                .collect()
        })
        .unwrap_or_else(|| {
            vec![
                ("Tahap 1: Observasi".into(), "Amati fenomena".into()),
                ("Tahap 2: Proses Inti".into(), "Mekanisme utama".into()),
                ("Tahap 3: ?".into(), "Tentukan hasilnya".into()),
                ("Tahap 4: Kesimpulan".into(), "Evaluasi data".into()),
            ]
        });

    let n = steps.len().max(1);
    let mut inner = String::new();

    if layout == "cycle" && n == 4 {
        let positions = [(400.0, 140.0), (620.0, 255.0), (400.0, 365.0), (180.0, 255.0)];
        let arrows = [
            (498.0, 148.0, 610.0, 210.0),
            (610.0, 298.0, 498.0, 358.0),
            (302.0, 358.0, 190.0, 298.0),
            (190.0, 210.0, 302.0, 148.0),
        ];
        for (x1, y1, x2, y2) in arrows {
            inner.push_str(&format!(
                r##"<line x1="{x1:.1}" y1="{y1:.1}" x2="{x2:.1}" y2="{y2:.1}" stroke="#0284C7" stroke-width="2.5" marker-end="url(#arrow-blue)"/>"##
            ));
        }
        for (idx, ((cx, cy), (lbl, note))) in positions.iter().zip(steps.iter()).enumerate() {
            let is_q = lbl.contains('?');
            let (fill, stroke, text_col) = if is_q {
                ("#FEF3C7", "#D97706", "#92400E")
            } else {
                ("#EFF6FF", PALETTE[idx % PALETTE.len()], "#0F172A")
            };
            let dash = if is_q { r#" stroke-dasharray="6 4""# } else { "" };
            let rx = cx - 95.0;
            let ry = cy - 34.0;
            inner.push_str(&format!(
                r##"<rect x="{rx:.1}" y="{ry:.1}" width="190" height="68" rx="12" fill="{fill}" stroke="{stroke}" stroke-width="2.5"{dash}/>
                   <text x="{cx:.1}" y="{ty1:.1}" text-anchor="middle" font-size="13.5" font-weight="800" fill="{text_col}">{lbl}</text>
                   <text x="{cx:.1}" y="{ty2:.1}" text-anchor="middle" font-size="11" font-weight="500" fill="#475569">{note}</text>"##,
                ty1 = cy - 4.0,
                ty2 = cy + 16.0,
                lbl = escape_xml(&clip_label(lbl, 24)),
                note = escape_xml(&clip_label(note, 28))
            ));
        }
    } else {
        let gap = 24.0;
        let total_w = 720.0;
        let box_w = ((total_w - gap * (n as f64 - 1.0)) / n as f64).clamp(115.0, 195.0);
        let used_w = box_w * n as f64 + gap * (n as f64 - 1.0);
        let start_x = 400.0 - used_w / 2.0;
        let by = 185.0;
        let bh = 128.0;

        for (idx, (lbl, note)) in steps.iter().enumerate() {
            let bx = start_x + idx as f64 * (box_w + gap);
            let cx = bx + box_w / 2.0;
            let is_q = lbl.contains('?');
            let accent = if is_q { "#D97706" } else { PALETTE[idx % PALETTE.len()] };
            let fill = if is_q { "#FEF3C7" } else { "#FFFFFF" };
            let dash = if is_q { r#" stroke-dasharray="6 4""# } else { "" };

            inner.push_str(&format!(
                r##"<rect x="{bx:.1}" y="{by:.1}" width="{box_w:.1}" height="{bh:.1}" rx="12" fill="{fill}" stroke="{accent}" stroke-width="2.5"{dash}/>
                   <circle cx="{cx:.1}" cy="{ny:.1}" r="14" fill="{accent}"/>
                   <text x="{cx:.1}" y="{nty:.1}" text-anchor="middle" font-size="12" font-weight="800" fill="#FFFFFF">{num}</text>
                   <text x="{cx:.1}" y="{ly:.1}" text-anchor="middle" font-size="13" font-weight="800" fill="#0F172A">{lbl}</text>
                   <text x="{cx:.1}" y="{sy:.1}" text-anchor="middle" font-size="11" font-weight="500" fill="#64748B">{note}</text>"##,
                ny = by + 26.0,
                nty = by + 30.0,
                num = idx + 1,
                ly = by + 64.0,
                sy = by + 88.0,
                lbl = escape_xml(&clip_label(lbl, 20)),
                note = escape_xml(&clip_label(note, 22))
            ));

            if idx + 1 < n {
                let ax1 = bx + box_w + 3.0;
                let ax2 = bx + box_w + gap - 3.0;
                let ay = by + bh / 2.0;
                inner.push_str(&format!(
                    r##"<line x1="{ax1:.1}" y1="{ay:.1}" x2="{ax2:.1}" y2="{ay:.1}" stroke="#475569" stroke-width="2.5" marker-end="url(#arrow)"/>"##
                ));
            }
        }
    }

    svg_frame("BAGAN ALUR", title, None, &inner)
}

fn render_science_diagram(spec: &Value) -> String {
    let title = spec.get("title").and_then(|v| v.as_str()).unwrap_or("Diagram Konsep");
    let subtype = spec
        .get("subtype")
        .or_else(|| spec.get("diagram_type"))
        .or_else(|| spec.get("shape"))
        .and_then(|v| v.as_str())
        .unwrap_or("water_cycle")
        .trim()
        .to_lowercase();

    if subtype.contains("circuit") || subtype.contains("listrik") {
        let mode = spec.get("mode").and_then(|v| v.as_str()).unwrap_or("seri").to_lowercase();
        let voltage = escape_xml(&dim_str(spec.get("dimensions").unwrap_or(spec), &["v", "voltage", "tegangan"], "12 V"));
        let switch_lbl = escape_xml(spec.get("switch_label").and_then(|v| v.as_str()).unwrap_or("Sakelar S"));
        let is_parallel = mode.contains("paralel") || mode.contains("parallel");
        let inner = if is_parallel {
            format!(
                r##"<rect x="170" y="130" width="460" height="220" fill="none" stroke="#0F172A" stroke-width="3.5" rx="6"/>
                   <line x1="170" y1="240" x2="630" y2="240" stroke="#0F172A" stroke-width="3.5"/>
                   <rect x="132" y="205" width="76" height="70" rx="8" fill="#FEF3C7" stroke="#D97706" stroke-width="2.5"/>
                   <text x="170" y="236" text-anchor="middle" font-size="13" font-weight="800" fill="#92400E">Baterai</text>
                   <text x="170" y="256" text-anchor="middle" font-size="13" font-weight="800" fill="#B45309">{voltage}</text>
                   <circle cx="420" cy="130" r="24" fill="#FEF08A" stroke="#CA8A04" stroke-width="3"/>
                   <text x="420" y="135" text-anchor="middle" font-size="13" font-weight="800" fill="#713F12">L1</text>
                   <circle cx="420" cy="240" r="24" fill="#FEF08A" stroke="#CA8A04" stroke-width="3"/>
                   <text x="420" y="245" text-anchor="middle" font-size="13" font-weight="800" fill="#713F12">L2</text>
                   <rect x="335" y="332" width="170" height="34" rx="8" fill="#E0F2FE" stroke="#0284C7" stroke-width="2"/>
                   <text x="420" y="354" text-anchor="middle" font-size="13" font-weight="800" fill="#0369A1">{switch_lbl}</text>"##
            )
        } else {
            format!(
                r##"<rect x="180" y="135" width="440" height="210" fill="none" stroke="#0F172A" stroke-width="3.5" rx="8"/>
                   <rect x="138" y="205" width="84" height="70" rx="8" fill="#FEF3C7" stroke="#D97706" stroke-width="2.5"/>
                   <text x="180" y="236" text-anchor="middle" font-size="13" font-weight="800" fill="#92400E">Baterai</text>
                   <text x="180" y="256" text-anchor="middle" font-size="13" font-weight="800" fill="#B45309">{voltage}</text>
                   <circle cx="330" cy="135" r="26" fill="#FEF08A" stroke="#CA8A04" stroke-width="3"/>
                   <text x="330" y="140" text-anchor="middle" font-size="14" font-weight="800" fill="#713F12">L1</text>
                   <circle cx="480" cy="135" r="26" fill="#FEF08A" stroke="#CA8A04" stroke-width="3"/>
                   <text x="480" y="140" text-anchor="middle" font-size="14" font-weight="800" fill="#713F12">L2</text>
                   <rect x="315" y="328" width="170" height="34" rx="8" fill="#E0F2FE" stroke="#0284C7" stroke-width="2"/>
                   <text x="400" y="350" text-anchor="middle" font-size="13" font-weight="800" fill="#0369A1">{switch_lbl}</text>"##
            )
        };
        return svg_frame("DIAGRAM RANGKAIAN", title, Some(if is_parallel { "Rangkaian Paralel" } else { "Rangkaian Seri" }), &inner);
    }

    if subtype.contains("lever") || subtype.contains("tuas") || subtype.contains("pengungkit") {
        let dims = spec.get("dimensions").unwrap_or(spec);
        let w = escape_xml(&dim_str(dims, &["w", "beban", "load"], "600 N"));
        let f = escape_xml(&dim_str(dims, &["f", "kuasa", "effort"], "? N"));
        let lb = escape_xml(&dim_str(dims, &["lb", "lengan_beban"], "50 cm"));
        let lk = escape_xml(&dim_str(dims, &["lk", "lengan_kuasa"], "150 cm"));
        let inner = format!(
            r##"<line x1="145" y1="265" x2="655" y2="265" stroke="#0F172A" stroke-width="8" stroke-linecap="round"/>
               <polygon points="335,269 305,335 365,335" fill="#D97706" stroke="#92400E" stroke-width="2.5"/>
               <text x="335" y="362" text-anchor="middle" font-size="13" font-weight="800" fill="#92400E">Titik Tumpu</text>
               <rect x="135" y="195" width="76" height="62" rx="8" fill="#DBEAFE" stroke="#0284C7" stroke-width="2.5"/>
               <text x="173" y="232" text-anchor="middle" font-size="14" font-weight="800" fill="#0369A1">W = {w}</text>
               <line x1="635" y1="185" x2="635" y2="252" stroke="#059669" stroke-width="4" marker-end="url(#arrow-emerald)"/>
               <text x="635" y="172" text-anchor="middle" font-size="14.5" font-weight="800" fill="#047857">F = {f}</text>
               <line x1="173" y1="295" x2="335" y2="295" stroke="#64748B" stroke-width="2"/>
               <text x="254" y="315" text-anchor="middle" font-size="13" font-weight="700" fill="#334155">lb = {lb}</text>
               <line x1="335" y1="295" x2="635" y2="295" stroke="#64748B" stroke-width="2"/>
               <text x="485" y="315" text-anchor="middle" font-size="13" font-weight="700" fill="#334155">lk = {lk}</text>"##
        );
        return svg_frame("DIAGRAM TUAS", title, None, &inner);
    }

    if subtype.contains("compass") || subtype.contains("map") || subtype.contains("peta") {
        let is_en = title.to_lowercase().contains("map") || title.to_lowercase().contains("direction");
        let (u, t, s, b) = if is_en { ("North (N)", "East (E)", "South (S)", "West (W)") } else { ("Utara (U)", "Timur (T)", "Selatan (S)", "Barat (B)") };
        let nodes: Vec<(String, String, String)> = spec
            .get("nodes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|n| {
                        let name = n.get("name").or_else(|| n.get("label")).and_then(|v| v.as_str())?.trim().to_string();
                        let pos = n.get("direction").or_else(|| n.get("dir")).and_then(|v| v.as_str()).unwrap_or("U").to_string();
                        let dist = n.get("distance").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        Some((name, pos, dist))
                    })
                    .take(4)
                    .collect()
            })
            .unwrap_or_else(|| {
                if is_en {
                    vec![
                        ("Library".into(), "N".into(), "2 km".into()),
                        ("Market".into(), "E".into(), "3 km".into()),
                        ("School".into(), "S".into(), "1.5 km".into()),
                        ("Park".into(), "W".into(), "2.5 km".into()),
                    ]
                } else {
                    vec![
                        ("Sekolah".into(), "U".into(), "2 km".into()),
                        ("Pasar".into(), "T".into(), "3 km".into()),
                        ("Perpustakaan".into(), "S".into(), "1.5 km".into()),
                        ("Balai Desa".into(), "B".into(), "2.5 km".into()),
                    ]
                }
            });

        let mut inner = format!(
            r##"<line x1="400" y1="125" x2="400" y2="375" stroke="#94A3B8" stroke-width="2" stroke-dasharray="5 4"/>
               <line x1="210" y1="250" x2="590" y2="250" stroke="#94A3B8" stroke-width="2" stroke-dasharray="5 4"/>
               <circle cx="400" cy="250" r="18" fill="#0284C7"/>
               <text x="400" y="255" text-anchor="middle" font-size="11" font-weight="800" fill="#FFFFFF">Pusat</text>
               <text x="400" y="114" text-anchor="middle" font-size="12.5" font-weight="800" fill="#0369A1">{u}</text>
               <text x="615" y="254" text-anchor="start" font-size="12.5" font-weight="800" fill="#0369A1">{t}</text>
               <text x="400" y="398" text-anchor="middle" font-size="12.5" font-weight="800" fill="#0369A1">{s}</text>
               <text x="190" y="254" text-anchor="end" font-size="12.5" font-weight="800" fill="#0369A1">{b}</text>"##
        );
        let slots = [(400.0, 165.0), (540.0, 250.0), (400.0, 335.0), (260.0, 250.0)];
        for (idx, (name, _, dist)) in nodes.iter().enumerate() {
            let (cx, cy) = slots[idx % 4];
            let sub = if dist.is_empty() { name.clone() } else { format!("{name} ({dist})") };
            inner.push_str(&format!(
                r##"<rect x="{rx:.1}" y="{ry:.1}" width="146" height="42" rx="10" fill="#FFFFFF" stroke="#059669" stroke-width="2"/>
                   <text x="{cx:.1}" y="{ty:.1}" text-anchor="middle" font-size="12.5" font-weight="800" fill="#0F172A">{lbl}</text>"##,
                rx = cx - 73.0,
                ry = cy - 21.0,
                ty = cy + 5.0,
                lbl = escape_xml(&clip_label(&sub, 22))
            ));
        }
        return svg_frame("PETA &amp; ARAH", title, None, &inner);
    }

    // Default water_cycle / food_chain -> clean cycle flowchart
    let fallback_steps = if subtype.contains("food") || subtype.contains("rantai") {
        json!({
            "title": title,
            "layout": "linear",
            "steps": [
                { "label": "Produsen", "note": "Tumbuhan hijau" },
                { "label": "Konsumen I", "note": "Herbivora" },
                { "label": "Konsumen II", "note": "Karnivora" },
                { "label": "Pengurai", "note": "Bakteri / Jamur" }
            ]
        })
    } else {
        let mut s = spec.clone();
        if s.get("steps").is_none() {
            s["layout"] = Value::String("cycle".to_string());
            s["steps"] = json!([
                { "label": "1. Evaporasi", "note": "Penguapan air laut" },
                { "label": "2. Kondensasi", "note": "Membentuk awan" },
                { "label": "3. Presipitasi", "note": "Turunnya hujan" },
                { "label": "4. Infiltrasi", "note": "Penyerapan ke tanah" }
            ]);
        }
        s
    };
    render_flowchart(&fallback_steps)
}

pub fn render_diagram_svg(spec: &Value) -> String {
    let kind = spec
        .get("type")
        .or_else(|| spec.get("kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("flowchart")
        .trim()
        .to_lowercase();

    match kind.as_str() {
        "bar_chart" | "bar" | "diagram_batang" => render_bar_chart(spec),
        "line_chart" | "line" | "grafik_garis" => render_line_chart(spec),
        "pie_chart" | "pie" | "diagram_lingkaran" => render_pie_chart(spec),
        "geometry" | "geometri" | "shape" => render_geometry(spec),
        "science_diagram" | "science" | "map" | "compass_map" | "circuit" | "lever" => {
            render_science_diagram(spec)
        }
        _ => render_flowchart(spec),
    }
}

/// Persists rendered SVG diagram into `uploaded_files` table + `uploads/` disk cache and returns `/uploads/<filename>`.
pub async fn persist_diagram_svg(pool: &PgPool, svg_content: &str) -> String {
    let filename = format!("ai_diagram_{}.svg", Uuid::new_v4());
    let bytes = svg_content.as_bytes();

    let db_res = sqlx::query(
        r#"
        INSERT INTO uploaded_files (filename, mime_type, file_data)
        VALUES ($1, 'image/svg+xml', $2)
        ON CONFLICT (filename) DO UPDATE
        SET file_data = EXCLUDED.file_data, mime_type = EXCLUDED.mime_type, created_at = NOW()
        "#,
    )
    .bind(&filename)
    .bind(bytes)
    .execute(pool)
    .await;

    if let Err(e) = db_res {
        tracing::warn!("Failed persisting SVG diagram to uploaded_files: {e}");
    }

    let _ = tokio::fs::create_dir_all("uploads").await;
    let disk_path = std::path::PathBuf::from("uploads").join(&filename);
    let _ = tokio::fs::write(&disk_path, bytes).await;

    format!("/uploads/{filename}")
}

/// Resolves a `diagram_spec` into a persisted `/uploads/ai_diagram_...svg` URL (or Cloudflare FLUX illustration if requested).
pub async fn resolve_visual_diagram_url(
    pool: &PgPool,
    diagram_spec: Option<&Value>,
    subject_name: &str,
    grade_level: &str,
) -> Option<String> {
    let spec = diagram_spec?;
    if !spec.is_object() {
        return None;
    }

    let kind = spec
        .get("type")
        .or_else(|| spec.get("kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase();

    if kind == "illustration" {
        let prompt = spec
            .get("prompt")
            .or_else(|| spec.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !prompt.is_empty() {
            if let Some(scene_url) =
                crate::infrastructure::cloudflare_ai::try_generate_scene_image(pool, prompt, subject_name, grade_level).await
            {
                return Some(scene_url);
            }
        }
    }

    let svg = render_diagram_svg(spec);
    Some(persist_diagram_svg(pool, &svg).await)
}

/// Builds a self-consistent visual question (question_text, 4 choices, correct_idx, explanation, diagram_spec)
/// used for fallback paths or when AI omitted `diagram_spec` despite `include_images = true`.
pub fn build_verified_visual_mcq(
    subject_name: &str,
    topic: &str,
    is_english: bool,
) -> (String, [String; 4], usize, String, Value) {
    let lower_subj = subject_name.trim().to_lowercase();
    let lower_topic = topic.trim().to_lowercase();

    if is_english {
        let spec = json!({
            "type": "bar_chart",
            "title": format!("Student Reading Log: {topic}"),
            "x_label": "Day",
            "y_label": "Pages Read",
            "bars": [
                { "label": "Monday", "value": 12 },
                { "label": "Tuesday", "value": 18 },
                { "label": "Wednesday", "value": 24 },
                { "label": "Thursday", "value": 15 }
            ]
        });
        return (
            format!("Observe the bar chart above about '{topic}'. On which day did students read the highest number of pages, and how many pages did they read?"),
            [
                "Monday with 12 pages".to_string(),
                "Tuesday with 18 pages".to_string(),
                "Wednesday with 24 pages".to_string(),
                "Thursday with 15 pages".to_string(),
            ],
            2,
            "Looking carefully at the bar chart, the tallest bar is Wednesday showing 24 pages.".to_string(),
            spec,
        );
    }

    if lower_subj.contains("matematika") || lower_topic.contains("segitiga") || lower_topic.contains("luas") || lower_topic.contains("geometri") {
        let spec = json!({
            "type": "geometry",
            "shape": "right_triangle",
            "title": format!("Bangun Datar Segitiga Siku-Siku: {topic}"),
            "labels": ["A", "B", "C"],
            "dimensions": { "a": "6 cm", "b": "8 cm", "c": "? cm" }
        });
        return (
            "Perhatikan gambar segitiga siku-siku ABC di atas! Jika panjang sisi alas AB/BC = 6 cm dan tinggi = 8 cm, berapakah luas daerah segitiga tersebut?".to_string(),
            [
                "14 cm²".to_string(),
                "24 cm²".to_string(),
                "28 cm²".to_string(),
                "48 cm²".to_string(),
            ],
            1,
            "Luas segitiga = 1/2 × alas × tinggi = 1/2 × 6 cm × 8 cm = 24 cm².".to_string(),
            spec,
        );
    }

    if lower_topic.contains("listrik") || lower_topic.contains("rangkaian") {
        let spec = json!({
            "type": "science_diagram",
            "subtype": "circuit",
            "mode": "seri",
            "title": "Skema Rangkaian Listrik Dasar",
            "dimensions": { "v": "12 V" },
            "switch_label": "Sakelar S (Tertutup)"
        });
        return (
            "Perhatikan gambar rangkaian listrik seri di atas! Apabila salah satu lampu (L1) terputus atau dilepas dari dudukannya sementara sakelar S tertutup, apa yang terjadi pada lampu L2?".to_string(),
            [
                "Lampu L2 menyala lebih terang".to_string(),
                "Lampu L2 ikut padam karena arus listrik terputus".to_string(),
                "Lampu L2 berkedip secara bergantian".to_string(),
                "Baterai otomatis berhenti bekerja permanen".to_string(),
            ],
            1,
            "Pada rangkaian seri hanya terdapat satu jalur arus listrik, sehingga jika L1 putus maka aliran ke L2 ikut terputus dan L2 padam.".to_string(),
            spec,
        );
    }

    if lower_subj.contains("ipa") || lower_subj.contains("ipas") || lower_subj.contains("sains") || lower_subj.contains("biologi") || lower_topic.contains("air") || lower_topic.contains("siklus") {
        let spec = json!({
            "type": "flowchart",
            "layout": "cycle",
            "title": format!("Siklus Air & Perubahan Alam: {topic}"),
            "steps": [
                { "label": "1. Evaporasi", "note": "Penguapan air laut" },
                { "label": "2. ?", "note": "Uap menjadi awan" },
                { "label": "3. Presipitasi", "note": "Turunnya hujan" },
                { "label": "4. Infiltrasi", "note": "Menyerap ke tanah" }
            ]
        });
        return (
            "Perhatikan bagan siklus air di atas! Proses perubahan uap air menjadi titik-titik air pembentuk awan pada tahap nomor 2 yang bertanda '?' disebut...".to_string(),
            [
                "Evaporasi".to_string(),
                "Kondensasi".to_string(),
                "Presipitasi".to_string(),
                "Sublimasi".to_string(),
            ],
            1,
            "Setelah evaporasi (penguapan), uap air mendingin di atmosfer menjadi awan melalui proses kondensasi.".to_string(),
            spec,
        );
    }

    let spec = json!({
        "type": "bar_chart",
        "title": format!("Grafik Pengamatan: {topic}"),
        "x_label": "Kelompok Pengamatan",
        "y_label": "Nilai / Skor",
        "bars": [
            { "label": "Tahap A", "value": 15 },
            { "label": "Tahap B", "value": 30 },
            { "label": "Tahap C", "value": 45 },
            { "label": "Tahap D", "value": 25 }
        ]
    });
    (
        format!("Perhatikan grafik batang hasil pengamatan tentang '{topic}' di atas! Pada tahap manakah nilai tertinggi tercatat beserta angkanya?"),
        [
            "Tahap A dengan nilai 15".to_string(),
            "Tahap B dengan nilai 30".to_string(),
            "Tahap C dengan nilai 45".to_string(),
            "Tahap D dengan nilai 25".to_string(),
        ],
        2,
        "Berdasarkan grafik batang di atas, batang tertinggi terdapat pada Tahap C dengan nilai tepat 45.".to_string(),
        spec,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_precise_bar_chart_geometry_and_flowchart_svgs() {
        let bar = render_diagram_svg(&json!({
            "type": "bar_chart",
            "title": "Hasil Panen & Data <Uji>",
            "bars": [
                { "label": "Padi", "value": 40 },
                { "label": "Jagung", "value": 25 }
            ]
        }));
        assert!(bar.starts_with("<svg "));
        assert!(bar.contains("40"));
        assert!(bar.contains("25"));
        assert!(bar.contains("&lt;Uji&gt;"));

        let tri = render_diagram_svg(&json!({
            "type": "geometry",
            "shape": "right_triangle",
            "title": "Segitiga Siku",
            "labels": ["P", "Q", "R"],
            "dimensions": { "a": "9 cm", "b": "12 cm", "c": "? cm" }
        }));
        assert!(tri.contains("9 cm"));
        assert!(tri.contains("12 cm"));
        assert!(tri.contains("? cm"));

        let flow = render_diagram_svg(&json!({
            "type": "flowchart",
            "layout": "cycle",
            "title": "Siklus",
            "steps": [
                { "label": "1. Mulai", "note": "A" },
                { "label": "2. ?", "note": "B" },
                { "label": "3. Lanjut", "note": "C" },
                { "label": "4. Akhir", "note": "D" }
            ]
        }));
        assert!(flow.contains("2. ?"));
        assert!(flow.contains("</svg>"));
    }
}
