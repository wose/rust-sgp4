/*!  # Simplified Perturbations Models (SGP4)

The _Simplified Perturbations Models_ are a set of models used for
satellites and objects relative to the Earth-centered inertial coordinate
system. These are often referred to collectively as **SGP4** because of how
popular that particular code is and how it's used with nearly all low Earth
orbit satellites.

The SGP4 and SDP4 models were published as FORTRAN IV in 1988. It has also
been ported to C. This is a port to Rust.

Original paper: [Hoots_Roehrich_1980_SPACETRACK_REPORT_NO_3.pdf](../Hoots_Roehrich_1980_SPACETRACK_REPORT_NO_3.pdf)
*/
#![deny(
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications,
)]

// TODO: Think about names
#![allow(
    non_upper_case_globals,
    non_snake_case,
)]


pub mod tle;
pub mod coordinates;

use std::io::Write;


macro_rules! println_stderr(
    ($($arg:tt)*) => { {
        let r = writeln!(&mut ::std::io::stderr(), $($arg)*);
        r.expect("failed printing to stderr");
    } }
);


/// $k_e = 7.43669161 \times 10\^{-2}$  Orbital constant for Earth defined as $\sqrt{GM_{\oplus}}$ where $G$ is Newton’s universal gravitational constant and $M_{\oplus}$ is the mass of the Earth. Units: $(\frac{\mathrm{Earth\ radii}}{\mathrm{minute}})\^{\frac{3}{2}}$
pub const ke: f64 = 7.43669161e-2;

/// $k_2 = 5.413080 \times 10\^{-4}$  Harmonic gravity constant for the SGP4 model. Defined as $\frac{1}{2}J_2aE\^2$.
pub const k2: f64 = 5.413080e-4;

/// $R_\oplus = 1.0$  Radius of the Earth (in Earth Radii).
pub const RE: f64 = 1.0;

/// $6378.135$ kilometers/Earth radii.
pub const XKMPER: f64 = 6378.135;

/// S (?)
pub const S: f64 = 1.01222928;

/// qs4 (?)
pub const qs4: f64 = 1.88027916e-9;


/// ## Propagate
///
/// Propagate the orbit to the desired time.
pub fn propagate(tle: tle::TLE, time: f64) -> coordinates::TEME {

    // Copy from NORAD elements
    let n0 = tle.mean_motion;
    let i0 = tle.i;
    let e0 = tle.e;

    // Pre-compute expensive things
    let cos_i0 = i0.cos();
    let cos2_i0 = cos_i0 * cos_i0;
    let e02 = e0 * e0;


    // ************************************************************************
    // Section 1.
    // Convert from NORAD (TLE) mean elements to SGP4 elements.

    // We go through two iterations of refining aₒ (semi-major axis) and
    // nₒ (mean motion)

    //       kₑ  ⅔
    // a₁ = ----
    //       nₒ
    let a1 = (ke/n0).powf(2.0/3.0);

    //      3 k₂   (3 cos² iₒ - 1)
    // δ₁ = - --- ----------------
    //      2 a₁²   (1 - eₒ²)³/₂
    let d1 = (3.0 * k2  * ( 3.0 * cos2_i0 - 1.0)) / (2.0 * a1 * a1 * ( 1.0 - e02).powf(3.0/2.0));

    //         ⌈     1           134    ⌉
    // aₒ = a₁ | 1 - -δ₁ - δ₁² - ---δ₁³ |
    //         ⌊     3            81    ⌋
    let a0 = a1 * ( 1.0 - (d1/3.0) - (d1 * d1) - (134.0 * d1 * d1 * d1 / 81.0));

    //      3 k₂   (3 cos² iₒ - 1)
    // δₒ = - --- ----------------
    //      2 aₒ²   (1 - eₒ²)³/₂
    let d0 = (3.0 * k2  * ( 3.0 * cos2_i0 - 1.0)) / (2.0 * a0 * a0 * ( 1.0 - e02).powf(3.0/2.0));

    //          nₒ
    // nₒ" = --------
    //       (1 + δₒ)
    let n0_dp = n0 / (1.0 + d0);

    //          aₒ
    // aₒ" = --------
    //       (1 - δₒ)
    let ao_dp = a0 / (1.0 - d0);


    // ************************************************************************
    // Section 2.
    // Determine apogee and perigee so we can deicide which SGP4 variant to
    // use later.

    // p = [aₒ"(1 - eₒ) - Rₑ] * XKMPER
    let perigee = (ao_dp * (1.0 - e0) - RE) * XKMPER;

    // p = [aₒ"(1 + eₒ) - Rₑ] * XKMPER
    let apogee = (ao_dp * (1.0 + e0) - RE) * XKMPER;


    // ************************************************************************
    // Section 3.
    // Calculate more constants

    // Set parameter "s" depending on perigee of the satellite:
    let s: f64;
    if perigee < 156.0 {
        // s = aₒ"(1 − eₒ) − s + aE
        s = ao_dp * (1.0 - e0) - S + RE;
    }
    else if perigee < 98.0 {
        s = (20.0 / XKMPER) + RE;
    }
    else {
        // For everything else use original value of s
        s = S;
    }

    // θ = cos iₒ
    let O = cos_i0;
    let O2 = O * O;

    //        1
    // ξ = -------
    //     aₒ" - s
    let xi = 1.0 / (ao_dp - s);
    let xi4 = xi.powi(4);

    //               ½
    // βₒ = (1 − eₒ²)
    let B = (1.0 - e02).sqrt();

    // η = aₒ"eₒξ
    let n = ao_dp * e0 * xi;
    let n2 = n.powi(2);
    let n3 = n.powi(3);
    let n4 = n.powi(4);

    //                           -⁷/₂⌈   ⌈    3                ⌉   3   k₂ξ    ⌈ 1   3  ⌉                ⌉
    // C₂ = (qₒ − s)⁴ξ⁴nₒ"(1 - η²)   |aₒ"|1 + -η² + 4eₒη + eₒη³| + - -------- |-- + -θ²|(8 + 24η² + 3η⁴)|
    //                               ⌊   ⌊    2                ⌋   2 (1 - η²) ⌊ 2   2  ⌋                ⌋
    let C2 = qs4 * xi4 * n0_dp * (1.0 - n2).powf(-7.0/2.0) * (ao_dp * (1.0 + (1.5 * n2) + (4.0 * e0 * n) + (e0 * n3)) + 1.5 * (k2 * xi)/(1.0 - n2) * (-0.5 + (1.5 * O2)) * (8.0 + (24.0 * n2) + (3.0 * n4)));


    // TODO: dummy
    // Return coordinates
    coordinates::TEME {
        X: 0.0,
        Y: 0.0,
        Z: 0.0,
    }
}

#[cfg(test)]
mod tests {

    use tle::load_from_str;
    use coordinates::TEME;
    use super::propagate;

    #[test]
    fn spacetrack_report_3_sgp4_test_case() {
        // This testcase is from "SPACETRACK REPORT NO. 3, Models for
        // Propagation of NORAD Element Sets, Hoots & Roehrich 1980
        // pg. 81:
        let tle = load_from_str(
            "Test",
            "1 88888U          80275.98708465  .00073094  13844-3  66816-4 0     8",
            "2 88888  72.8435 115.9689 0086731  52.6988 110.5714 16.05824518   105",
        );

        // Compute
        let result0 = propagate(tle, 0.0);
        assert_eq!(result0, TEME {
            X: 0.0,
            Y: 0.0,
            Z: 0.0,
        });

    }
}
