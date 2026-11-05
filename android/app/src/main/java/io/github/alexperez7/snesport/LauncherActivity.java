package io.github.alexperez7.snesport;

import android.app.Activity;
import android.app.AlertDialog;
import android.os.Handler;
import android.os.Looper;
import android.view.View;
import android.widget.CheckBox;
import android.widget.ProgressBar;
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

    private static final String ACTION_SETTINGS = "io.github.alexperez7.snesport.SETTINGS";
    private boolean started;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        boolean haveRom = new File(getFilesDir(), "rom.sfc").isFile();
        if (haveRom && !ACTION_SETTINGS.equals(getIntent().getAction())) {
            // Abrir la app va directo al juego; solo con el ajuste de actualizaciones activo se mira antes si hay versión nueva.
            if (UpdateChecker.enabled(this)) checkThenStart();
            else startGame();
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
        pick.setText(haveRom ? "Cambiar ROM" : "Elegir ROM");
        pick.setOnClickListener(v -> {
            Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            i.addCategory(Intent.CATEGORY_OPENABLE);
            i.setType("*/*");
            startActivityForResult(i, PICK_ROM);
        });
        Button licences = new Button(this);
        licences.setText("Licencias");
        licences.setOnClickListener(v -> showLicences());
        col.addView(title);
        col.addView(message);
        if (haveRom) {
            message.setText("Versión " + UpdateChecker.installedVersion(this));
            Button play = new Button(this);
            play.setText("Jugar");
            play.setOnClickListener(v -> startGame());
            col.addView(play);
        }
        col.addView(pick);
        if (UpdateChecker.available(this)) {
            CheckBox auto = new CheckBox(this);
            auto.setText("Buscar actualizaciones al abrir (usa Internet)");
            auto.setChecked(UpdateChecker.enabled(this));
            auto.setOnCheckedChangeListener((b, on) -> UpdateChecker.setEnabled(this, on));
            Button now = new Button(this);
            now.setText("Buscar actualizaciones ahora");
            now.setOnClickListener(v -> checkNow(now));
            col.addView(auto);
            col.addView(now);
        }
        col.addView(licences);
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

    /** Con el ajuste activo: espera como mucho 3 s a la red (una consulta al día) y abre el juego pase lo que pase. */
    private void checkThenStart() {
        if (UpdateChecker.fresh(this)) {
            decide(UpdateChecker.fromCache(this));
            return;
        }
        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);
        col.setGravity(Gravity.CENTER);
        TextView t = new TextView(this);
        t.setText("Buscando actualizaciones...");
        t.setGravity(Gravity.CENTER);
        t.setPadding(0, 32, 0, 0);
        col.addView(new ProgressBar(this));
        col.addView(t);
        setContentView(col);
        final boolean[] done = {false};
        Handler ui = new Handler(Looper.getMainLooper());
        ui.postDelayed(() -> { if (!done[0]) { done[0] = true; startGame(); } }, 3000);
        new Thread(() -> {
            UpdateChecker.Result r = UpdateChecker.query(this);
            ui.post(() -> { if (!done[0]) { done[0] = true; decide(r); } });
        }).start();
    }

    /** Si hay una versión nueva que no se ha omitido, pregunta; si no, abre el juego. */
    private void decide(UpdateChecker.Result r) {
        if (r.ok && r.newer && !UpdateChecker.skipped(this, r.tag)) showUpdateDialog(r, this::startGame);
        else startGame();
    }

    /** «Descargar» abre la página de la release; «Ahora no» y «Omitir» siguen con `after`. */
    private void showUpdateDialog(UpdateChecker.Result r, Runnable after) {
        new AlertDialog.Builder(this)
            .setTitle("Versión nueva")
            .setMessage("Hay una versión nueva: " + r.tag + " (tienes " + UpdateChecker.installedVersion(this) + ").")
            .setPositiveButton("Descargar", (d, w) -> {
                startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(r.url)));
                finish();
            })
            .setNegativeButton("Ahora no", (d, w) -> after.run())
            .setNeutralButton("Omitir esta versión", (d, w) -> { UpdateChecker.skip(this, r.tag); after.run(); })
            .setOnCancelListener(d -> after.run())
            .show();
    }

    /** El botón de los ajustes: consulta ahora y dice en pantalla qué ha pasado. */
    private void checkNow(Button button) {
        button.setEnabled(false);
        message.setText("Buscando actualizaciones...");
        Handler ui = new Handler(Looper.getMainLooper());
        new Thread(() -> {
            UpdateChecker.Result r = UpdateChecker.query(this);
            ui.post(() -> {
                button.setEnabled(true);
                if (!r.ok) message.setText("No se pudo comprobar. Revisa la conexión e inténtalo de nuevo.");
                else if (!r.newer) message.setText("Estás al día (versión " + UpdateChecker.installedVersion(this) + ").");
                else {
                    message.setText("Hay una versión nueva: " + r.tag + " (tienes " + UpdateChecker.installedVersion(this) + ").");
                    showUpdateDialog(r, () -> {});
                }
            });
        }).start();
    }

    /** Los avisos de terceros (LGPL de libtcc y zlib de SDL2) con el texto de las licencias, tal como van en los assets. */
    private void showLicences() {
        StringBuilder sb = new StringBuilder();
        try {
            for (String f : new String[] {"NOTICE.txt", "libtcc-LGPL-2.1.txt", "SDL2-zlib.txt"}) {
                try (InputStream in = getAssets().open("licenses/" + f)) {
                    sb.append(new String(in.readAllBytes(), StandardCharsets.UTF_8)).append("\n\n----\n\n");
                }
            }
        } catch (IOException e) {
            sb.append("No se encuentran los avisos de licencia: ").append(e.getMessage());
        }
        TextView text = new TextView(this);
        text.setText(sb);
        text.setPadding(32, 32, 32, 32);
        android.widget.ScrollView scroll = new android.widget.ScrollView(this);
        scroll.addView(text);
        new android.app.AlertDialog.Builder(this).setTitle("Licencias").setView(scroll).setPositiveButton("Cerrar", null).show();
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
        if (started) return;
        started = true;
        startActivity(new Intent(this, GameActivity.class));
        finish();
    }
}
