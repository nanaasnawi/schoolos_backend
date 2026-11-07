const { Client } = require('pg');

const client = new Client({
  connectionString: 'postgresql://postgres:ePssELIUrkhPIlsGqKvIgGvMuDodFsYM@altaria.proxy.rlwy.net:21200/railway',
  ssl: false
});

async function run() {
  await client.connect();

  const class9A = '01a10ca3-d5a7-7041-859b-5358599cf0a4';
  const class9B = '01a10ca3-d184-7a23-b116-7fd945b46f78';
  const class8A = '01a10ca3-ce12-7c23-a3b9-e1c976370c01';
  const class8B = '01a10ca3-caa2-7861-b081-9fb19b0f3109';

  console.log('--- SYNCING EKOSISTEM (IPA) TO PAKET B 9B ---');
  // Check if 9B has Ekosistem material
  const m9b = await client.query("SELECT id FROM learning_materials WHERE class_id = $1 AND title ILIKE '%Ekosistem%'", [class9B]);
  if (m9b.rows.length === 0) {
    const sourceM = await client.query("SELECT * FROM learning_materials WHERE class_id = $1 AND title ILIKE '%Ekosistem%' LIMIT 1", [class9A]);
    if (sourceM.rows.length > 0) {
      const src = sourceM.rows[0];
      await client.query(`
        INSERT INTO learning_materials (
          id, tenant_id, lesson_id, material_type, title, description, storage_key, external_url,
          order_index, visibility, is_active, class_id, subject_id, teacher_id, created_by,
          source_type, library_book_id, start_page, end_page, session_id, release_at, notification_dispatched
        ) VALUES (
          gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21
        )
      `, [
        src.tenant_id, src.lesson_id, src.material_type, src.title, src.description, src.storage_key, src.external_url,
        src.order_index, src.visibility, true, class9B, src.subject_id, src.teacher_id, src.created_by,
        src.source_type, src.library_book_id, src.start_page, src.end_page, src.session_id, src.release_at, src.notification_dispatched
      ]);
      console.log('Inserted Ekosistem material for 9B');
    }
  } else {
    console.log('Ekosistem material already exists for 9B');
  }

  // Check if 9B has Ekosistem assignment
  const a9b = await client.query("SELECT id FROM assignments WHERE class_id = $1 AND title ILIKE '%Ekosistem%'", [class9B]);
  if (a9b.rows.length === 0) {
    const sourceA = await client.query("SELECT * FROM assignments WHERE class_id = $1 AND title ILIKE '%Ekosistem%' LIMIT 1", [class9A]);
    if (sourceA.rows.length > 0) {
      const src = sourceA.rows[0];
      const newAssIdRes = await client.query(`
        INSERT INTO assignments (
          id, tenant_id, lesson_id, title, description, instructions, max_score, due_at,
          assignment_type, status, is_active, class_id, subject_id, teacher_id, created_by,
          session_id, release_at, allow_late_submission, notification_dispatched
        ) VALUES (
          gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18
        ) RETURNING id
      `, [
        src.tenant_id, src.lesson_id, src.title, src.description, src.instructions, src.max_score, src.due_at,
        src.assignment_type, src.status, true, class9B, src.subject_id, src.teacher_id, src.created_by,
        src.session_id, src.release_at, src.allow_late_submission, src.notification_dispatched
      ]);
      const newAssId = newAssIdRes.rows[0].id;
      // Copy questions & choices
      const srcQs = await client.query("SELECT * FROM assignment_questions WHERE assignment_id = $1", [src.id]);
      for (const q of srcQs.rows) {
        const newQRes = await client.query(`
          INSERT INTO assignment_questions (id, tenant_id, assignment_id, question_text, question_type, points, order_index)
          VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6) RETURNING id
        `, [q.tenant_id, newAssId, q.question_text, q.question_type, q.points, q.order_index]);
        const newQId = newQRes.rows[0].id;
        const srcCs = await client.query("SELECT * FROM assignment_question_choices WHERE question_id = $1", [q.id]);
        for (const c of srcCs.rows) {
          await client.query(`
            INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
            VALUES (gen_random_uuid(), $1, $2, $3, $4)
          `, [newQId, c.choice_text, c.is_correct, c.order_index]);
        }
      }
      console.log('Inserted Ekosistem assignment & questions for 9B');
    }
  } else {
    console.log('Ekosistem assignment already exists for 9B');
  }

  console.log('--- SYNCING RECOUNT TEXT (BAHASA INGGRIS) TO PAKET B 8B ---');
  // Check if 8B has Recount Text material
  const m8b = await client.query("SELECT id FROM learning_materials WHERE class_id = $1 AND title ILIKE '%Recount Text%'", [class8B]);
  if (m8b.rows.length === 0) {
    const sourceM = await client.query("SELECT * FROM learning_materials WHERE class_id = $1 AND title ILIKE '%Recount Text%' AND is_active = true LIMIT 1", [class8A]);
    if (sourceM.rows.length > 0) {
      const src = sourceM.rows[0];
      await client.query(`
        INSERT INTO learning_materials (
          id, tenant_id, lesson_id, material_type, title, description, storage_key, external_url,
          order_index, visibility, is_active, class_id, subject_id, teacher_id, created_by,
          source_type, library_book_id, start_page, end_page, session_id, release_at, notification_dispatched
        ) VALUES (
          gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21
        )
      `, [
        src.tenant_id, src.lesson_id, src.material_type, src.title, src.description, src.storage_key, src.external_url,
        src.order_index, src.visibility, true, class8B, src.subject_id, src.teacher_id, src.created_by,
        src.source_type, src.library_book_id, src.start_page, src.end_page, src.session_id, src.release_at, src.notification_dispatched
      ]);
      console.log('Inserted Recount Text material for 8B');
    }
  }

  // Check if 8B has Recount Text assignment
  const a8b = await client.query("SELECT id FROM assignments WHERE class_id = $1 AND title ILIKE '%Recount Text%'", [class8B]);
  if (a8b.rows.length === 0) {
    const sourceA = await client.query("SELECT * FROM assignments WHERE class_id = $1 AND title ILIKE '%Recount Text%' LIMIT 1", [class8A]);
    if (sourceA.rows.length > 0) {
      const src = sourceA.rows[0];
      const newAssIdRes = await client.query(`
        INSERT INTO assignments (
          id, tenant_id, lesson_id, title, description, instructions, max_score, due_at,
          assignment_type, status, is_active, class_id, subject_id, teacher_id, created_by,
          session_id, release_at, allow_late_submission, notification_dispatched
        ) VALUES (
          gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18
        ) RETURNING id
      `, [
        src.tenant_id, src.lesson_id, src.title, src.description, src.instructions, src.max_score, src.due_at,
        src.assignment_type, 'published', true, class8B, src.subject_id, src.teacher_id, src.created_by,
        src.session_id, src.release_at, src.allow_late_submission, src.notification_dispatched
      ]);
      const newAssId = newAssIdRes.rows[0].id;
      // Copy questions & choices
      const srcQs = await client.query("SELECT * FROM assignment_questions WHERE assignment_id = $1", [src.id]);
      for (const q of srcQs.rows) {
        const newQRes = await client.query(`
          INSERT INTO assignment_questions (id, tenant_id, assignment_id, question_text, question_type, points, order_index)
          VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6) RETURNING id
        `, [q.tenant_id, newAssId, q.question_text, q.question_type, q.points, q.order_index]);
        const newQId = newQRes.rows[0].id;
        const srcCs = await client.query("SELECT * FROM assignment_question_choices WHERE question_id = $1", [q.id]);
        for (const c of srcCs.rows) {
          await client.query(`
            INSERT INTO assignment_question_choices (id, question_id, choice_text, is_correct, order_index)
            VALUES (gen_random_uuid(), $1, $2, $3, $4)
          `, [newQId, c.choice_text, c.is_correct, c.order_index]);
        }
      }
      // Also ensure 8A assignment is active/published
      await client.query("UPDATE assignments SET status = 'published', is_active = true WHERE id = $1", [src.id]);
      console.log('Inserted Recount Text assignment for 8B and activated for 8A');
    }
  }

  await client.end();
}

run().catch(console.error);
