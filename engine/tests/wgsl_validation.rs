use naga::valid::{Capabilities, ValidationFlags, Validator};

const SHADERS: &[(&str, &str)] = &[
    (
        "terrain.wgsl",
        include_str!("../src/renderer/shaders/terrain.wgsl"),
    ),
    (
        "route.wgsl",
        include_str!("../src/renderer/shaders/route.wgsl"),
    ),
];

#[test]
fn all_wgsl_shaders_parse_and_validate() {
    for (name, source) in SHADERS {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|error| panic!("{name} failed to parse: {error}"));
        Validator::new(ValidationFlags::all(), Capabilities::all())
            .validate(&module)
            .unwrap_or_else(|error| panic!("{name} failed validation: {error}"));
    }
}
