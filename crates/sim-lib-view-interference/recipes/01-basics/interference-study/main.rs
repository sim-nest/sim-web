fn main() -> sim_kernel::Result<()> {
    for line in sim_lib_view_interference::cookbook::interference_study_demo()? {
        println!("{line}");
    }
    Ok(())
}
