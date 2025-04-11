use anyhow::Result;

fn main() -> Result<()> {
    let s = vec![1i32, 2i32];
    let s = s.into_iter().next().ok_or(anyhow::anyhow!("labalaba"))?;
    println!("{:?}", s);
    Ok(())
}
