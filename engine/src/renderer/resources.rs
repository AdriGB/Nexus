use crate::world::Grid;

pub fn encode_resource_texture(grid: &Grid) -> Vec<u8> {
    let mut data = Vec::with_capacity(grid.resources.len() * 4);

    for deposit in &grid.resources {
        match deposit {
            Some(deposit) => {
                data.push(deposit.kind as u8);
                data.push(deposit.amount as u8);
                data.push((deposit.amount >> 8) as u8);
                data.push(255);
            }
            None => data.extend_from_slice(&[0, 0, 0, 0]),
        }
    }

    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ResourceDeposit, ResourceKind, Terrain, Tile};

    #[test]
    fn encodes_kind_and_u16_amount() {
        let grid = Grid {
            width: 1,
            height: 1,
            tiles: vec![Tile {
                terrain: Terrain::Forest,
                altitude: 0.0,
                moisture: 0.5,
                temperature: 0.5,
            }],
            region_ids: Vec::new(),
            regions: Vec::new(),
            resources: vec![Some(ResourceDeposit {
                kind: ResourceKind::Timber,
                amount: 840,
            })],
            renewable_resources: Vec::new(),
        };

        assert_eq!(encode_resource_texture(&grid), vec![2, 72, 3, 255]);
    }
}
