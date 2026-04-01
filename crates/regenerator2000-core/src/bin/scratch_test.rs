fn main() {
    let mut state = regenerator2000_core::state::AppState::new();
    let res = state.load_file(std::path::PathBuf::from(
        "/Users/ricardoq/progs/regenerator2000/tests/6502/A2-Amper-fdraw/AMPERFDRAW#061d60.dis65",
    ));
    if res.is_err() {
        println!("Failed to load: {:?}", res.err());
        return;
    }

    let skip2_addr = regenerator2000_core::state::Addr(8074);
    let skip_addr = regenerator2000_core::state::Addr(8076);
    let loop_addr = regenerator2000_core::state::Addr(8054);

    println!("skip2 at 8074: {:?}", state.labels.get(&skip2_addr));
    println!("skip at 8076: {:?}", state.labels.get(&skip_addr));
    println!("loop at 8054: {:?}", state.labels.get(&loop_addr));
    println!(
        "Xrefs to skip(8076): {:?}",
        state.cross_refs.get(&skip_addr)
    );
    println!(
        "Xrefs to skip2(8074): {:?}",
        state.cross_refs.get(&skip2_addr)
    );
}
