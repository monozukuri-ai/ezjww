use ezjww_core::{Coord2D, JwwWriteDocument};

/// Reproducible inputs for the native application oracle (also used by tests).
pub fn drawing(name: &str) -> Option<JwwWriteDocument> {
    let mut doc = JwwWriteDocument::default();
    match name {
        "empty" => {}
        "line" => {
            doc.add_line(Coord2D::new(0.0, 0.0), Coord2D::new(100.0, 0.0));
        }
        "basic" => {
            let line = doc.add_line(Coord2D::new(-50.0, -20.0), Coord2D::new(50.0, -20.0));
            line.base.pen_style = 3;
            line.base.pen_color = 3;
            line.base.pen_width = 25;
            line.base.layer = 2;
            doc.add_circle(Coord2D::new(-30.0, 20.0), 10.0)
                .base
                .pen_color = 2;
            doc.add_arc(Coord2D::new(0.0, 20.0), 10.0, 350.0, 30.0);
            doc.add_point(Coord2D::new(30.0, 20.0)).base.pen_color = 4;
            let text = doc.add_text(
                Coord2D::new(-50.0, -40.0),
                Coord2D::new(-20.0, -40.0),
                "日本語 ABC",
            );
            text.spacing = 0.5;
            text.base.pen_color = 5;
            let text = doc.add_text(
                Coord2D::new(30.0, -40.0),
                Coord2D::new(30.0, -20.0),
                "縦方向",
            );
            text.angle = 90.0;
            text.size_y = 5.0;
        }
        "settings" => {
            doc.options.paper_size = 4;
            doc.options.memo = "図面設定の検証\r\n".into();
            doc.options.write_layer_group = 2;
            for (g, group) in doc.options.layer_groups.iter_mut().enumerate() {
                group.name = format!("グループ{g:X}");
                group.state = if g == 2 { 3 } else { (g % 3) as u32 };
                group.write_layer = 5;
                group.scale = if g == 2 { 50.0 } else { 0.5 };
                group.protect = if g == 4 {
                    1
                } else if g == 5 {
                    2
                } else {
                    0
                };
                for (l, layer) in group.layers.iter_mut().enumerate() {
                    layer.name = format!("層{g:X}-{l:X}");
                    layer.state = if l == 5 { 3 } else { (l % 3) as u32 };
                    layer.protect = if l == 8 {
                        1
                    } else if l == 9 {
                        2
                    } else {
                        0
                    };
                }
            }
            let line = doc.add_line(Coord2D::new(-10.0, -5.0), Coord2D::new(10.0, 5.0));
            line.base.layer_group = 2;
            line.base.layer = 5;
            let circle = doc.add_circle(Coord2D::new(0.0, 0.0), 2.0);
            circle.base.layer_group = 15;
            circle.base.layer = 15;
        }
        "unicode" => {
            doc.options.memo = "日本語𠮷\r\n".into();
            doc.options.layer_groups[0].name = "図面𠮷".into();
            doc.options.layer_groups[0].layers[2].name = "日本語の層".repeat(60);
            for (i, content) in [
                "".into(),
                "日本語𠮷ｶﾅ ABC".into(),
                "長".repeat(255),
                "文".repeat(1024),
            ]
            .into_iter()
            .enumerate()
            {
                doc.add_text(
                    Coord2D::new(0.0, i as f64 * 10.0),
                    Coord2D::new(20.0, i as f64 * 10.0),
                    content,
                );
            }
        }
        _ => return None,
    }
    Some(doc)
}
