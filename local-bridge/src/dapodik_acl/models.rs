use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DapodikRawStudent {
    pub peserta_didik_id: Option<String>,
    pub nipd: Option<String>,
    pub nisn: Option<String>,
    pub nik: Option<String>,
    pub nama: Option<String>,
    pub nama_pd: Option<String>,
    pub rombel: Option<String>,
    pub nama_rombel: Option<String>,
    pub jenis_kelamin: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<String>,
    pub agama_id_str: Option<String>,
    pub nama_ayah: Option<String>,
    pub pekerjaan_ayah_id_str: Option<String>,
    #[serde(alias = "nama_ibu_kandung", alias = "nama_ibu_kandung_str", alias = "ibu_kandung")]
    pub nama_ibu: Option<String>,
    pub pekerjaan_ibu_id_str: Option<String>,
    pub nama_wali: Option<String>,
    pub pekerjaan_wali_id_str: Option<String>,
    pub nomor_telepon_seluler: Option<String>,
    pub nomor_telepon_rumah: Option<String>,
    pub alamat_jalan: Option<String>,
    pub email: Option<String>,
    pub jenis_keluar_id: Option<serde_json::Value>,
    pub jenis_keluar_id_str: Option<String>,
    pub tanggal_keluar: Option<String>,
    pub keterangan_keluar: Option<String>,
    pub status: Option<String>,
    pub status_di_sekolah: Option<String>,
    pub aktif: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DapodikRawGtk {
    pub ptk_id: Option<String>,
    pub nip: Option<String>,
    pub nuptk: Option<String>,
    pub nik: Option<String>,
    pub nama: Option<String>,
    pub nama_ptk: Option<String>,
    pub nama_gtk: Option<String>,
    pub jenis_ptk: Option<String>,
    pub jenis_ptk_id_str: Option<String>,
    pub mata_pelajaran: Option<String>,
    pub mapel: Option<String>,
    pub jenis_kelamin: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<String>,
    pub agama_id_str: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DapodikRawPembelajaran {
    pub pembelajaran_id: Option<String>,
    pub mata_pelajaran_id: Option<serde_json::Value>,
    pub mata_pelajaran_id_str: Option<String>,
    pub nama_mata_pelajaran: Option<String>,
    pub ptk_id: Option<String>,
    pub jam_mengajar_per_minggu: Option<serde_json::Value>,
    pub status_di_kurikulum_str: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DapodikRawRombel {
    pub rombongan_belajar_id: Option<String>,
    pub nama: Option<String>,
    pub ptk_id: Option<String>,
    pub tingkat_pendidikan_id: Option<String>,
    pub jenis_rombel: Option<serde_json::Value>,
    pub jenis_rombel_str: Option<String>,
    pub pembelajaran: Option<Vec<DapodikRawPembelajaran>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentSyncPayload {
    pub dapodik_url: Option<String>,
    pub npsn: Option<String>,
    pub bearer_token: Option<String>,
    pub raw_students: Option<Vec<DapodikRawStudent>>,
    pub raw_gtk: Option<Vec<DapodikRawGtk>>,
    pub raw_rombel: Option<Vec<DapodikRawRombel>>,
    pub raw_sekolah: Option<serde_json::Value>,
}

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
    pub jenis_kelamin: String,
    pub nik: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: String,
    
    pub agama_id: Option<i32>,
    pub agama_id_str: Option<String>,
    pub alamat_jalan: Option<String>,
    pub nomor_telepon_rumah: Option<String>,
    pub nomor_telepon_seluler: Option<String>,
    
    pub nama_ayah: Option<String>,
    #[serde(alias = "nama_ibu_kandung", alias = "nama_ibu_kandung_str", alias = "ibu_kandung")]
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

