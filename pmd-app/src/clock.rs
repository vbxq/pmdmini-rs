// SPDX-License-Identifier: AGPL-3.0-only

#[cfg(not(target_arch = "wasm32"))]
pub fn now_seconds() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_secs_f64()
}

#[cfg(target_arch = "wasm32")]
pub fn now_seconds() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or(0.0, |performance| performance.now() / 1_000.0)
}

#[cfg(test)]
mod tests {
    use super::now_seconds;

    #[test]
    fn the_clock_moves_forward_and_never_backward() {
        let first = now_seconds();
        let mut last = first;
        for _ in 0..1_000 {
            let now = now_seconds();
            assert!(now >= last, "the clock went backwards: {now} after {last}");
            last = now;
        }
        assert!(last >= first);
    }
}
