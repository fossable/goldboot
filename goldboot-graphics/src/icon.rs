use crate::*;

#[rustfmt::skip]
const ICON_MATRIX: [[u32; 7]; 7] = [
	[0, 0, 0, 0, 1, 0, 0],
	[0, 0, 0, 0, 1, 0, 0],
	[1, 1, 1, 0, 1, 1, 1],
	[1, 0, 1, 0, 1, 0, 1],
	[1, 1, 1, 0, 1, 1, 1],
	[0, 0, 1, 0, 0, 0, 0],
	[1, 1, 1, 0, 0, 0, 0],
];

const ICON_MARGIN: usize = 7;

pub fn icon_svg(background: bool) -> Svg {
    let mut svg = Svg::default();
    svg.width = format!(
        "{}",
        (ICON_MARGIN * 2)
            + ((ICON_MATRIX.first().unwrap().len() - 1) * (RECT_SIDE + RECT_GAP))
            + RECT_SIDE
            - adjust_horizontal(ICON_MATRIX.first().unwrap().len() - 1)
    );
    svg.height = format!(
        "{}",
        (ICON_MARGIN * 2) + ((ICON_MATRIX.len() - 1) * (RECT_SIDE + RECT_GAP)) + RECT_SIDE
    );

    // Add background
    if background {
        svg.g.rect.push(Rect {
            style: String::from(BG_STYLE),
            id: format!("background"),
            width: format!("100%"),
            height: format!("100%"),
            x: format!(""),
            y: format!(""),
            rx: String::from("8%"),
        });
    }

    for rect in generate_rects() {
        svg.g.rect.push(rect);
    }

    return svg;
}

pub fn generate_rects() -> Vec<Rect> {
    let mut rects = Vec::new();

    for r in 0..ICON_MATRIX.len() {
        for c in 0..ICON_MATRIX.first().unwrap().len() {
            if ICON_MATRIX[r][c] == 1 {
                rects.push(Rect {
                    style: String::from(RECT_STYLE),
                    id: format!("{r}-{c}"),
                    width: format!("{}", RECT_SIDE),
                    height: format!("{}", RECT_SIDE),
                    x: format!(
                        "{}",
                        ICON_MARGIN + (c * RECT_SIDE + c * RECT_GAP) - adjust_horizontal(c)
                    ),
                    y: format!("{}", ICON_MARGIN + (r * RECT_SIDE + r * RECT_GAP)),
                    rx: String::from("1"),
                });
            }
        }
    }

    return rects;
}
