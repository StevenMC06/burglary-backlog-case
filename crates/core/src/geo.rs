//! Geometry helpers.
use crate::types::GeoPoint;

/// Great-circle distance in kilometres (haversine, mean Earth radius 6371.0088 km).
pub fn haversine_km(a: GeoPoint, b: GeoPoint) -> f64 {
    let r = 6371.0088_f64;
    let (la1, la2) = (a.lat.to_radians(), b.lat.to_radians());
    let dlat = la2 - la1;
    let dlon = (b.lon - a.lon).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * r * h.sqrt().asin()
}
