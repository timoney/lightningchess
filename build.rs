use std::path::PathBuf;

fn main () -> Result<(), Box<dyn std::error::Error>> {

    let dir = "protos".to_string();
    let protos = vec![
        "lightning.proto",
        "invoicesrpc/invoices.proto"
    ];

    let proto_paths: Vec<_> = protos
        .iter()
        .map(|proto| {
            let mut path = PathBuf::from(&dir);
            path.push(proto);
            path.display().to_string()
        })
        .collect();

    tonic_build::configure()
        .build_client(true)
        .build_server(false)
        .compile(&proto_paths, &[dir])?;
    Ok(())
}