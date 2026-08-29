use serde::Deserialize;
use serde_json;

#[derive(Debug, Deserialize)]
pub struct DapodikRawGtk {
    pub ptk_id: Option<String>,
    #[serde(alias = "nuptk", alias = "nik")]
    pub nip: Option<String>,
    #[serde(alias = "nama_ptk", alias = "nama_gtk")]
    pub nama: Option<String>,
    #[serde(alias = "jenis_ptk", alias = "jenis_ptk_id_str", alias = "mata_pelajaran", alias = "mapel")]
    pub subject: Option<String>,
    pub jenis_kelamin: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<String>,
    pub agama_id_str: Option<String>,
}

fn main() {
    let json_str = include_str!("../../../scripts/gtk2.json");
    let v: serde_json::Value = serde_json::from_str(json_str).unwrap();

    let rows = v.get("rows").unwrap().clone();
    let res: Result<Vec<DapodikRawGtk>, _> = serde_json::from_value(rows);
    match res {
        Ok(data) => println!("Success: {} rows", data.len()),
        Err(e) => println!("Error: {}", e),
    }
}
