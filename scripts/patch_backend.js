const fs = require('fs');

// 1. Update classes controller.rs
const classFile = 'backend/api-server/src/presentation/academic/classes/controller.rs';
let classContent = fs.readFileSync(classFile, 'utf8');

const target1 = `            if !allowed_class_ids.is_empty() {
                items.retain(|c| allowed_class_ids.contains(&c.id));
            }`;
const target1CRLF = target1.replace(/\n/g, '\r\n');

if (classContent.includes(target1)) {
  classContent = classContent.replace(target1, '            // Akses rombel dibuka penuh untuk seluruh guru pengampu');
  fs.writeFileSync(classFile, classContent);
  console.log('Updated classes controller (LF)');
} else if (classContent.includes(target1CRLF)) {
  classContent = classContent.replace(target1CRLF, '            // Akses rombel dibuka penuh untuk seluruh guru pengampu');
  fs.writeFileSync(classFile, classContent);
  console.log('Updated classes controller (CRLF)');
} else {
  console.warn('target1 not found in classes controller');
}

// 2. Update library controller.rs to remove restriction
const libFile = 'backend/api-server/src/presentation/learning/library/controller.rs';
let libContent = fs.readFileSync(libFile, 'utf8');

const targetLib = `            if !is_homeroom && !is_scheduled {
                return Err(ApiError::new(
                    ApplicationError::Unauthorized(
                        ErrorCode::AuthPermissionDenied,
                        format!("Anda tidak mengampu kelas \\"{}\\". Penugasan materi buku SIBI hanya dapat dilakukan pada kelas yang diampu.", class_name),
                    ),
                    &req_ctx.request_id,
                ));
            }`;
const targetLibCRLF = targetLib.replace(/\n/g, '\r\n');

if (libContent.includes(targetLib)) {
  libContent = libContent.replace(targetLib, '            // Akses penugasan buku dibuka penuh ke seluruh kelas');
  fs.writeFileSync(libFile, libContent);
  console.log('Updated library controller (LF)');
} else if (libContent.includes(targetLibCRLF)) {
  libContent = libContent.replace(targetLibCRLF, '            // Akses penugasan buku dibuka penuh ke seluruh kelas');
  fs.writeFileSync(libFile, libContent);
  console.log('Updated library controller (CRLF)');
} else {
  console.warn('targetLib not found in library controller');
}

