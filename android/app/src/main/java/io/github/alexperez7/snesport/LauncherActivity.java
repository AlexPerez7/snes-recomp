package io.github.alexperez7.snesport;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.view.Gravity;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.zip.CRC32;

/** Pantalla de inicio: si ya hay una ROM válida guardada, juega; si no, la pide con el selector de documentos. */
public class LauncherActivity extends Activity {
    private static final int PICK_ROM = 1;
    private TextView message;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        if (new File(getFilesDir(), "rom.sfc").isFile()) {
            startGame();
            return;
        }
        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);
        col.setGravity(Gravity.CENTER);
        col.setPadding(48, 48, 48, 48);
        TextView title = new TextView(this);
        title.setText("The Legend of Zelda: A Link to the Past");
        title.setTextSize(20);
        title.setGravity(Gravity.CENTER);
        message = new TextView(this);
        message.setText("Elige tu copia de la ROM (USA, .sfc o .smc). Se queda en este teléfono y no se envía a ningún sitio.");
        message.setGravity(Gravity.CENTER);
        message.setPadding(0, 32, 0, 32);
        Button pick = new Button(this);
        pick.setText("Elegir ROM");
        pick.setOnClickListener(v -> {
            Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            i.addCategory(Intent.CATEGORY_OPENABLE);
            i.setType("*/*");
            startActivityForResult(i, PICK_ROM);
        });
        col.addView(title);
        col.addView(message);
        col.addView(pick);
        setContentView(col);
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != PICK_ROM || resultCode != RESULT_OK || data == null) return;
        try {
            Uri uri = data.getData();
            File tmp = new File(getFilesDir(), "rom.tmp");
            CRC32 crc = new CRC32();
            try (InputStream in = getContentResolver().openInputStream(uri); OutputStream out = new FileOutputStream(tmp)) {
                byte[] buf = new byte[1 << 16];
                for (int n; (n = in.read(buf)) > 0; ) {
                    crc.update(buf, 0, n);
                    out.write(buf, 0, n);
                }
            }
            String want = expectedCrc();
            String got = String.format("%08X", crc.getValue());
            if (!got.equalsIgnoreCase(want)) {
                tmp.delete();
                message.setText("Esta ROM no es la esperada (CRC32 " + got + ", se esperaba " + want + "). Prueba con la versión USA sin cabecera.");
                return;
            }
            File rom = new File(getFilesDir(), "rom.sfc");
            rom.delete();
            if (!tmp.renameTo(rom)) throw new IOException("no puedo guardar la ROM");
            startGame();
        } catch (IOException e) {
            message.setText("No he podido leer la ROM: " + e.getMessage());
        }
    }

    private String expectedCrc() throws IOException {
        try (InputStream in = getAssets().open("game/profile.toml")) {
            String toml = new String(in.readAllBytes(), StandardCharsets.UTF_8);
            Matcher m = Pattern.compile("crc32\\s*=\\s*\"([0-9A-Fa-f]{8})\"").matcher(toml);
            if (!m.find()) throw new IOException("el perfil no tiene crc32");
            return m.group(1);
        }
    }

    private void startGame() {
        startActivity(new Intent(this, GameActivity.class));
        finish();
    }
}
