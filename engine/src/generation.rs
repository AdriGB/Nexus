use noise::{NoiseFn, Perlin, Seedable};
use crate::world::{Grid, Tile, Terrain};

/// Genera un mundo completo usando múltiples capas de ruido Perlin
pub fn generate_world(seed: u32, width: u32, height: u32, sea_level: f64) -> Grid {
    // Diferentes semillas para cada capa de ruido
    let continent_noise = Perlin::default().set_seed(seed);
    let detail_noise = Perlin::default().set_seed(seed.wrapping_add(1));
    let moisture_noise = Perlin::default().set_seed(seed.wrapping_add(2));
    let temp_noise = Perlin::default().set_seed(seed.wrapping_add(3));
    let variation_noise = Perlin::default().set_seed(seed.wrapping_add(4));

    let mut tiles = Vec::with_capacity((width * height) as usize);

    for y in 0..height {
        for x in 0..width {
            let fx = x as f64;
            let fy = y as f64;

            // ── Altitud ──
            // Capa continental: baja frecuencia, crea masas de tierra grandes
            let continent = fbm(&continent_noise, fx, fy, 0.0018, 4, 0.5, 2.0);
            // Capa de detalle: frecuencia media, crea montañas y valles
            let detail = fbm(&detail_noise, fx, fy, 0.007, 6, 0.48, 2.1);
            // Combinar
            let altitude = continent * 0.58 + detail * 0.42;

            // ── Humedad ──
            let raw_moisture = fbm(&moisture_noise, fx, fy, 0.005, 4, 0.5, 2.0);
            let moisture = ((raw_moisture + 1.0) / 2.0).clamp(0.0, 1.0);

            // ── Temperatura ──
            // Basada en latitud (frío en polos) + variación por ruido
            let latitude = (fy / height as f64 - 0.5).abs() * 2.0;
            let temp_var = fbm(&temp_noise, fx, fy, 0.0025, 3, 0.4, 2.0) * 0.18;
            let temperature = (1.0 - latitude * 0.85 + temp_var).clamp(0.0, 1.0);

            // ── Variación sutil para textura ──
            let _variation = fbm(&variation_noise, fx, fy, 0.02, 2, 0.5, 2.0);

            // ── Clasificar terreno ──
            let terrain = classify_terrain(altitude, moisture, temperature, sea_level);

            tiles.push(Tile {
                terrain,
                altitude,
                moisture,
                temperature,
            });
        }
    }

    Grid {
        width,
        height,
        tiles,
    }
}

/// Fractal Brownian Motion: múltiples octavas de ruido para terreno natural
fn fbm(noise: &Perlin, x: f64, y: f64, base_freq: f64, octaves: usize, persistence: f64, lacunarity: f64) -> f64 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = base_freq;
    let mut max_amplitude = 0.0;

    for _ in 0..octaves {
        total += noise.get([x * frequency, y * frequency]) * amplitude;
        max_amplitude += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    total / max_amplitude
}

/// Clasifica un tile según sus propiedades ambientales
fn classify_terrain(altitude: f64, moisture: f64, temperature: f64, sea_level: f64) -> Terrain {
    // ── Océano profundo ──
    if altitude < sea_level - 0.18 {
        return Terrain::DeepWater;
    }

    // ── Agua poco profunda ──
    if altitude < sea_level {
        return Terrain::ShallowWater;
    }

    // ── Playa ──
    if altitude < sea_level + 0.025 {
        return Terrain::Beach;
    }

    // ── Cumbres nevadas ──
    if altitude > 0.72 {
        return Terrain::SnowPeak;
    }

    // ── Montañas ──
    if altitude > 0.52 {
        return Terrain::Mountain;
    }

    // ── Colinas ──
    if altitude > 0.40 {
        return Terrain::Hills;
    }

    // ── Tundra (frío extremo) ──
    if temperature < 0.18 {
        return Terrain::Tundra;
    }

    // ── Desierto (seco y cálido) ──
    if moisture < 0.18 && temperature > 0.4 {
        return Terrain::Desert;
    }

    // ── Pantano (muy húmedo y bajo) ──
    if moisture > 0.72 && altitude < sea_level + 0.08 {
        return Terrain::Swamp;
    }

    // ── Vegetación según humedad ──
    if moisture < 0.32 {
        Terrain::Plains
    } else if moisture < 0.48 {
        Terrain::Grassland
    } else if moisture < 0.65 {
        Terrain::Forest
    } else {
        Terrain::DenseForest
    }
}
