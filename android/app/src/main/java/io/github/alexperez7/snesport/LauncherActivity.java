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
import android.widget.FrameLayout;
import android.view.ViewGroup;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.atomic.AtomicBoolean;
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
        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);
        col.setGravity(Gravity.CENTER_HORIZONTAL);
        TextView title = ZeldaUi.text(this, getString(R.string.app_name), 30, ZeldaUi.GOLD_HI, true);
        title.setShadowLayer(4, 2, 2, ZeldaUi.OUTLINE);
        TextView subtitle = ZeldaUi.text(this, "The Legend of Zelda: A Link to the Past", 14, ZeldaUi.TEXT, false);
        message = ZeldaUi.text(this, getString(R.string.pick_message), 14, ZeldaUi.TEXT, false);
        message.setPadding(0, ZeldaUi.dp(this, 18), 0, ZeldaUi.dp(this, 6));
        Button pick = ZeldaUi.button(this, getString(haveRom ? R.string.change_rom : R.string.pick_rom), !haveRom);
        pick.setOnClickListener(v -> {
            Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            i.addCategory(Intent.CATEGORY_OPENABLE);
            i.setType("*/*");
            startActivityForResult(i, PICK_ROM);
        });
        Button licences = ZeldaUi.button(this, getString(R.string.licences), false);
        licences.setOnClickListener(v -> showLicences());
        View tri = new ZeldaUi.Triforce(this);
        col.addView(tri, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        title.setPadding(0, ZeldaUi.dp(this, 14), 0, ZeldaUi.dp(this, 2));
        col.addView(title);
        col.addView(subtitle);
        col.addView(message);
        if (haveRom) {
            message.setText(getString(R.string.version, UpdateChecker.installedVersion(this)));
            Button play = ZeldaUi.button(this, getString(R.string.play), true);
            play.setOnClickListener(v -> startGame());
            col.addView(play);
        }
        col.addView(pick);
        if (UpdateChecker.available(this)) {
            CheckBox auto = ZeldaUi.check(this, getString(R.string.auto_check), UpdateChecker.enabled(this));
            auto.setOnCheckedChangeListener((b, on) -> UpdateChecker.setEnabled(this, on));
            LinearLayout.LayoutParams alp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
            alp.topMargin = ZeldaUi.dp(this, 16);
            Button now = ZeldaUi.button(this, getString(R.string.check_now), false);
            now.setOnClickListener(v -> checkNow(now));
            col.addView(auto, alp);
            col.addView(now);
        }
        col.addView(licences);
        android.widget.ScrollView scroll = new android.widget.ScrollView(this);
        scroll.setFillViewport(true);
        FrameLayout.LayoutParams sp = new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT);
        int side = ZeldaUi.dp(this, 36);
        col.setPadding(side, ZeldaUi.dp(this, 48), side, ZeldaUi.dp(this, 36));
        col.setMinimumHeight(0);
        LinearLayout holder = new LinearLayout(this); // centra la columna cuando hay sitio de sobra
        holder.setGravity(Gravity.CENTER);
        holder.addView(col, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        scroll.addView(holder, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        ZeldaUi.Backdrop root = new ZeldaUi.Backdrop(this);
        root.addView(scroll, sp);
        View lang = new ZeldaUi.LangButton(this, Lang.get(this));
        lang.setOnClickListener(v -> { Lang.toggle(this); recreate(); });
        FrameLayout.LayoutParams lp = new FrameLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.TOP | Gravity.END);
        lp.topMargin = ZeldaUi.dp(this, 40);
        lp.rightMargin = ZeldaUi.dp(this, 40);
        root.addView(lang, lp);
        message.setMovementMethod(null);
        setContentView(root);
        // Siempre se pasa por el lanzador; con el ajuste activo se mira aquí si hay versión nueva (el juego espera a que se pulse «Jugar»).
        if (haveRom && UpdateChecker.enabled(this)) autoCheck();
    }

    @Override
    protected void attachBaseContext(android.content.Context base) {
        super.attachBaseContext(Lang.wrap(base));
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
                message.setText(getString(R.string.rom_bad, got, want));
                return;
            }
            File rom = new File(getFilesDir(), "rom.sfc");
            rom.delete();
            if (!tmp.renameTo(rom)) throw new IOException("no puedo guardar la ROM");
            recreate(); // de vuelta al lanzador, ya con «Jugar»
        } catch (IOException e) {
            message.setText(getString(R.string.rom_read_fail, e.getMessage()));
        }
    }

    /** Con el ajuste activo: una consulta al día (si no, lo último que se supo); si hay versión nueva que no se omitió, pregunta. */
    private void autoCheck() {
        if (UpdateChecker.fresh(this)) {
            offer(UpdateChecker.fromCache(this));
            return;
        }
        Handler ui = new Handler(Looper.getMainLooper());
        new Thread(() -> {
            UpdateChecker.Result r = UpdateChecker.query(this);
            ui.post(() -> offer(r));
        }).start();
    }

    private void offer(UpdateChecker.Result r) {
        if (isFinishing() || isDestroyed()) return;
        if (r.ok && r.newer && !UpdateChecker.skipped(this, r.tag)) {
            message.setText(getString(R.string.new_version, r.tag, UpdateChecker.installedVersion(this)));
            showUpdateDialog(r, () -> {});
        }
    }

    /** «Descargar e instalar» baja el APK de la release, comprueba su hash y lo instala; sin APK en la release abre su página. */
    private void showUpdateDialog(UpdateChecker.Result r, Runnable after) {
        new AlertDialog.Builder(this, R.style.ZeldaDialog)
            .setTitle(R.string.update_title)
            .setMessage(getString(R.string.new_version, r.tag, UpdateChecker.installedVersion(this)))
            .setPositiveButton(r.canInstall() ? R.string.update_install : R.string.update_open_page, (d, w) -> {
                if (r.canInstall()) installUpdate(r, after);
                else {
                    startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(r.url)));
                    finish();
                }
            })
            .setNegativeButton(R.string.update_later, (d, w) -> after.run())
            .setNeutralButton(R.string.update_skip, (d, w) -> { UpdateChecker.skip(this, r.tag); after.run(); })
            .setOnCancelListener(d -> after.run())
            .show();
    }

    /** Pide el permiso de instalar desde esta app si falta; si no, descarga con una barra de progreso y entrega el APK al sistema. */
    private void installUpdate(UpdateChecker.Result r, Runnable after) {
        if (!getPackageManager().canRequestPackageInstalls()) {
            new AlertDialog.Builder(this, R.style.ZeldaDialog)
                .setMessage(R.string.update_permission)
                .setPositiveButton(R.string.update_open_settings, (d, w) -> {
                    startActivity(new Intent(android.provider.Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:" + getPackageName())));
                    after.run();
                })
                .setNegativeButton(R.string.update_later, (d, w) -> after.run())
                .show();
            return;
        }
        ProgressBar bar = new ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal);
        bar.setMax(100);
        bar.setPadding(48, 32, 48, 0);
        AtomicBoolean cancel = new AtomicBoolean(false);
        AlertDialog dlg = new AlertDialog.Builder(this, R.style.ZeldaDialog)
            .setTitle(R.string.update_downloading)
            .setView(bar)
            .setCancelable(false)
            .setNegativeButton(R.string.cancel, (d, w) -> cancel.set(true))
            .show();
        Handler ui = new Handler(Looper.getMainLooper());
        new Thread(() -> {
            int error = 0;
            try {
                File apk = UpdateInstaller.download(this, r, p -> ui.post(() -> bar.setProgress(p)), cancel);
                UpdateInstaller.install(this, apk);
            } catch (UpdateInstaller.Failure f) {
                error = f.resId;
            } catch (IOException e) {
                error = R.string.upd_fail_net;
            }
            final int err = error;
            ui.post(() -> {
                dlg.dismiss();
                if (err != 0) {
                    android.widget.Toast.makeText(this, err, android.widget.Toast.LENGTH_LONG).show();
                    if (err == R.string.upd_cancelled) after.run();
                }
            });
        }).start();
    }

    /** El botón de los ajustes: consulta ahora y dice en pantalla qué ha pasado. */
    private void checkNow(Button button) {
        button.setEnabled(false);
        message.setText(R.string.checking);
        Handler ui = new Handler(Looper.getMainLooper());
        new Thread(() -> {
            UpdateChecker.Result r = UpdateChecker.query(this);
            ui.post(() -> {
                button.setEnabled(true);
                if (!r.ok) message.setText(R.string.check_failed);
                else if (!r.newer) message.setText(getString(R.string.up_to_date, UpdateChecker.installedVersion(this)));
                else {
                    message.setText(getString(R.string.new_version, r.tag, UpdateChecker.installedVersion(this)));
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
            sb.append(getString(R.string.licences_missing, e.getMessage()));
        }
        TextView text = new TextView(this);
        text.setText(sb);
        text.setTextColor(ZeldaUi.TEXT);
        text.setTextSize(13);
        text.setPadding(32, 32, 32, 32);
        android.widget.ScrollView scroll = new android.widget.ScrollView(this);
        scroll.addView(text);
        new AlertDialog.Builder(this, R.style.ZeldaDialog).setTitle(R.string.licences).setView(scroll).setPositiveButton(R.string.close, null).show();
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
