const { Client } = require('pg');

const DATABASE_URL = process.env.DATABASE_URL;
if (!DATABASE_URL) {
    console.error("FATAL: DATABASE_URL environment variable is required.");
    process.exit(1);
}

async function testParallelClassQueries() {
    const client = new Client({ connectionString: DATABASE_URL });
    await client.connect();

    console.log('--- Testing Parallel Class Resolution ---');

    // 1. Check classes and their tingkat
    const resClasses = await client.query(`
        SELECT id, name, tingkat, tenant_id FROM classes ORDER BY name;
    `);
    console.log('Classes found:', resClasses.rows.map(r => `${r.name} (tingkat: ${r.tingkat}, id: ${r.id})`));

    // 2. Pick a class that has parallel siblings: e.g. "PAKET B 8A"
    const class8A = resClasses.rows.find(r => r.name === 'PAKET B 8A');
    const class8B = resClasses.rows.find(r => r.name === 'PAKET B 8B');
    console.log('\nTesting with 8A:', class8A?.id, 'and 8B:', class8B?.id);

    // 3. Test sibling query: given class8A.id, find all sibling class IDs
    const siblingRes = await client.query(`
        SELECT sib.id, sib.name, sib.tingkat
        FROM classes c
        JOIN classes sib ON sib.tenant_id = c.tenant_id
             AND (sib.id = c.id OR (sib.tingkat IS NOT NULL AND sib.tingkat != '' AND sib.tingkat = c.tingkat))
        WHERE c.id = $1
    `, [class8A.id]);
    console.log('\nSiblings of 8A resolved:', siblingRes.rows.map(r => r.name));

    // 4. Test student matching query
    // Find students enrolled in 8A or 8B
    const studentsRes = await client.query(`
        SELECT s.id, s.user_id, s.full_name, c.name as class_name, en.status
        FROM enrollments en
        JOIN students s ON s.id = en.student_id
        JOIN classes c ON c.id = en.class_id
        WHERE c.id IN ($1, $2)
    `, [class8A.id, class8B.id]);
    console.log('\nStudents in 8A & 8B:', studentsRes.rows);

    // 5. Test access check query: given a student in 8B, does the query match an item published to 8A?
    const student8B = studentsRes.rows.find(s => s.class_name === 'PAKET B 8B') || studentsRes.rows[0];
    if (student8B) {
        console.log(`\nChecking student ${student8B.full_name} (${student8B.class_name}) access to 8A:`);
        const accessCheck = await client.query(`
            SELECT EXISTS (
                SELECT 1 
                FROM students s
                JOIN enrollments en ON en.student_id = s.id
                JOIN classes en_c ON en_c.id = en.class_id
                JOIN classes target_c ON target_c.id = $1
                WHERE s.id = $2
                  AND en.tenant_id = target_c.tenant_id
                  AND (
                      en_c.id = target_c.id 
                      OR (
                          target_c.tingkat IS NOT NULL 
                          AND target_c.tingkat != '' 
                          AND target_c.tingkat = en_c.tingkat
                      )
                  )
                  AND (en.status ILIKE 'active')
            ) as can_access
        `, [class8A.id, student8B.id]);
        console.log('Result can_access:', accessCheck.rows[0].can_access);
    }

    await client.end();
}

testParallelClassQueries().catch(console.error);
