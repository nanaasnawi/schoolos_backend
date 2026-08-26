use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DapodikWsStudent {
    pub registrasi_id: String,
    pub jenis_pendaftaran_id: Option<String>,
    pub jenis_pendaftaran_id_str: Option<String>,
    pub nipd: Option<String>,
    pub tanggal_masuk_sekolah: Option<String>,
    pub sekolah_asal: Option<String>,
    
    pub peserta_didik_id: String,
    pub nama: String,
    pub nisn: Option<String>,
    pub jenis_kelamin: String, // "L" or "P"
    pub nik: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: String,
    
    pub agama_id: Option<i32>,
    pub agama_id_str: Option<String>,
    pub alamat_jalan: Option<String>,
    pub nomor_telepon_rumah: Option<String>,
    pub nomor_telepon_seluler: Option<String>,
    
    pub nama_ayah: Option<String>,
    pub nama_ibu: Option<String>,
    pub nama_wali: Option<String>,
    
    pub email: Option<String>,
    pub semester_id: Option<String>,
    
    pub anggota_rombel_id: Option<String>,
    pub rombongan_belajar_id: Option<String>,
    pub tingkat_pendidikan_id: Option<String>,
    pub nama_rombel: Option<String>,
    pub kurikulum_id_str: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DapodikWsResponse {
    pub rows: Vec<DapodikWsStudent>,
}
