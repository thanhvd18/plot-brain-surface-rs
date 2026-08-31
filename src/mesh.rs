#[derive(Clone, Debug)]
pub struct SurfaceMesh {
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<[u32; 3]>,
}

impl SurfaceMesh {
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    pub fn merge_hemispheres(left: &SurfaceMesh, right: &SurfaceMesh) -> Self {
        let offset = left.vertices.len() as u32;
        let mut vertices = left.vertices.clone();
        vertices.extend_from_slice(&right.vertices);

        let mut faces = left.faces.clone();
        faces.extend(right.faces.iter().map(|face| {
            [
                face[0] + offset,
                face[1] + offset,
                face[2] + offset,
            ]
        }));

        Self { vertices, faces }
    }
}

#[derive(Clone, Debug)]
pub struct BrainSurfaces {
    pub left: SurfaceMesh,
    pub right: SurfaceMesh,
    pub both: SurfaceMesh,
}
