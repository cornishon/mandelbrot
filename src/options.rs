use std::str::FromStr;

#[derive(clap::Parser)]
pub struct Options {
    #[arg(short, long, value_parser = parse_pair::<u32, 'x'>, default_value = "1200x800")]
    pub window_size: (u32, u32),
    #[arg(short, long, value_parser = parse_pair::<f32, ','>, default_value = "-0.5,0.0")]
    pub center: (f32, f32),
    #[arg(short, long, default_value_t = 3.0)]
    pub zoom: f32,
}

fn parse_pair<T: FromStr, const SEP: char>(s: &str) -> Result<(T, T), String> {
    let Some((w, h)) = s.split_once(SEP) else {
        return Err(format!("expected argument in X{SEP}Y format"));
    };
    let w = w.parse().map_err(|_| "invalid X")?;
    let h = h.parse().map_err(|_| "invalid Y")?;
    Ok((w, h))
}
