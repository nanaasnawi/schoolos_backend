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
    #[serde(
        alias = "nama_ibu_kandung",
        alias = "nama_ibu_kandung_str",
        alias = "ibu_kandung"
    )]
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

    #[serde(alias = "rt", alias = "RT", alias = "r_t")]
    pub rt: Option<serde_json::Value>,
    #[serde(alias = "rw", alias = "RW", alias = "r_w")]
    pub rw: Option<serde_json::Value>,
    #[serde(alias = "dusun", alias = "nama_dusun", alias = "Dusun")]
    pub dusun: Option<String>,
    #[serde(alias = "kelurahan", alias = "desa_kelurahan", alias = "Kelurahan")]
    pub kelurahan: Option<String>,
    #[serde(alias = "kecamatan", alias = "nama_kecamatan", alias = "Kecamatan")]
    pub kecamatan: Option<String>,
    #[serde(alias = "kode_pos", alias = "kodepos", alias = "Kode Pos")]
    pub kode_pos: Option<String>,

    #[serde(alias = "pekerjaan_ayah", alias = "Pekerjaan Ayah")]
    pub pekerjaan_ayah: Option<String>,
    #[serde(alias = "penghasilan_ayah", alias = "penghasilan_ayah_id_str", alias = "Penghasilan Ayah")]
    pub penghasilan_ayah: Option<String>,

    #[serde(alias = "pekerjaan_ibu", alias = "Pekerjaan Ibu")]
    pub pekerjaan_ibu: Option<String>,
    #[serde(alias = "penghasilan_ibu", alias = "penghasilan_ibu_id_str", alias = "Penghasilan Ibu")]
    pub penghasilan_ibu: Option<String>,

    #[serde(alias = "pekerjaan_wali", alias = "Pekerjaan Wali")]
    pub pekerjaan_wali: Option<String>,
    #[serde(alias = "penghasilan_wali", alias = "penghasilan_wali_id_str", alias = "Penghasilan Wali")]
    pub penghasilan_wali: Option<String>,

    #[serde(alias = "penerima_kps", alias = "Penerima KPS", alias = "kps")]
    pub penerima_kps: Option<serde_json::Value>,
    #[serde(alias = "no_kps", alias = "nomor_kps", alias = "No. KPS")]
    pub no_kps: Option<String>,

    #[serde(alias = "penerima_kip", alias = "Penerima KIP", alias = "kip")]
    pub penerima_kip: Option<serde_json::Value>,
    #[serde(alias = "nomor_kip", alias = "no_kip", alias = "Nomor KIP")]
    pub nomor_kip: Option<String>,
    #[serde(alias = "nama_di_kip", alias = "nama_kip", alias = "Nama di KIP")]
    pub nama_di_kip: Option<String>,
    #[serde(alias = "nomor_kks", alias = "no_kks", alias = "Nomor KKS")]
    pub nomor_kks: Option<String>,

    #[serde(alias = "layak_pip", alias = "Layak PIP (usulan dari sekolah)", alias = "usulan_pip")]
    pub layak_pip: Option<serde_json::Value>,
    #[serde(alias = "alasan_layak_pip", alias = "Alasan Layak PIP", alias = "alasan_pip")]
    pub alasan_layak_pip: Option<String>,

    #[serde(alias = "bank", alias = "nama_bank", alias = "Bank")]
    pub bank: Option<String>,
    #[serde(alias = "nomor_rekening_bank", alias = "no_rekening", alias = "no_rekening_bank", alias = "Nomor Rekening Bank")]
    pub nomor_rekening_bank: Option<String>,
    #[serde(alias = "rekening_atas_nama", alias = "nama_rekening", alias = "Rekening Atas Nama")]
    pub rekening_atas_nama: Option<String>,
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
    pub jabatan_ptk_id_str: Option<String>,
    pub status_kepegawaian_id_str: Option<String>,
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
    pub synced_by: Option<String>,
    pub batch_index: Option<usize>,
    pub total_batches: Option<usize>,
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
    #[serde(
        alias = "nama_ibu_kandung",
        alias = "nama_ibu_kandung_str",
        alias = "ibu_kandung"
    )]
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
