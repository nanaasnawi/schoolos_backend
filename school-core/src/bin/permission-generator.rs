use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let permissions = vec![
        ("LearningAssignmentCreate", "Learning.Assignment.Create"),
        ("LearningAssignmentRead", "Learning.Assignment.Read"),
        ("LearningAssignmentUpdate", "Learning.Assignment.Update"),
        ("LearningAssignmentDelete", "Learning.Assignment.Delete"),
        ("AssessmentRead", "Assessment.Read"),
        ("AssessmentUpdate", "Assessment.Update"),
    ];

    // Path relatif dari folder 'backend' ke 'frontend'
    let target_dir = Path::new("../frontend/src/authorization");
    let target_file = target_dir.join("permission.ts");

    // 1. Buat foldernya dulu kalau belum ada (bikin anti-error NotFound)
    fs::create_dir_all(target_dir)?;

    // 2. Tulis filenya
    let mut file = File::create(&target_file)?;

    writeln!(
        file,
        "// ⚠️ AUTO-GENERATED FILE BY RUST BACKEND - DO NOT EDIT"
    )?;
    writeln!(file, "export const Permission = {{")?;

    for (key, val) in permissions {
        writeln!(file, "  {}: \"{}\",", key, val)?;
    }

    writeln!(file, "}} as const;\n")?;
    writeln!(
        file,
        "export type AppPermission = typeof Permission[keyof typeof Permission];"
    )?;

    println!("✅ Berhasil generate permission.ts ke {:?}", target_file);
    Ok(())
}
