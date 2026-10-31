package io.github.alexperez7.snesspike;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;

/**
 * Pantalla de la prueba: elegir la ROM (queda en el almacenamiento privado de la app, nunca sale del teléfono), generar
 * el C, medir y ver los resultados. Cada medición deja su carpeta en files/ con result.txt (y lo copia a logcat).
 */
public class MainActivity extends Activity {
    private static final int PICK_ROM = 1;
    private static final int FRAMES = 3000;
    private TextView log;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);
        col.setPadding(24, 24, 24, 24);
        button(col, "1. Elegir ROM", () -> {
            Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("*/*");
            startActivityForResult(i, PICK_ROM);
        });
        button(col, "2. Generar el C", this::generate);
        button(col, "3. Medir: en memoria, todos los núcleos", () -> bench("run_mem", 0, -1));
        button(col, "4. Medir: en memoria, núcleo 0 (clúster lento)", () -> bench("run_mem_cpu0", 0, 0));
        button(col, "5. Plan B: .so en files/ + dlopen", () -> bench("run_so", 1, -1));
        button(col, "Ver resultados", this::showResults);
        log = new TextView(this);
        log.setTextIsSelectable(true);
        col.addView(log);
        ScrollView scroll = new ScrollView(this);
        scroll.addView(col);
        setContentView(scroll);
    }

    @Override
    protected void onResume() {
        super.onResume();
        showResults();
    }

    private void button(LinearLayout col, String label, Runnable action) {
        Button b = new Button(this);
        b.setText(label);
        b.setAllCaps(false);
        b.setOnClickListener(v -> action.run());
        col.addView(b);
    }

    private void say(String s) {
        runOnUiThread(() -> log.setText(s + "\n\n" + log.getText()));
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != PICK_ROM || result != RESULT_OK || data == null) return;
        Uri uri = data.getData();
        new Thread(() -> {
            try (InputStream in = getContentResolver().openInputStream(uri);
                 OutputStream out = new FileOutputStream(new File(getFilesDir(), "rom.sfc"))) {
                long n = copy(in, out);
                say("ROM copiada a la app: " + n + " bytes");
            } catch (IOException e) {
                say("ERROR al leer la ROM: " + e);
            }
        }).start();
    }

    private void generate() {
        say("Generando…");
        new Thread(() -> {
            try {
                File files = getFilesDir();
                byte[] rom = Files.readAllBytes(new File(files, "rom.sfc").toPath());
                String toml = new String(asset("game/profile.toml"), StandardCharsets.UTF_8);
                List<String> names = new ArrayList<>(), texts = new ArrayList<>();
                String[] c = getAssets().list("game/c");
                for (String n : c == null ? new String[0] : c) {
                    names.add(n);
                    texts.add(new String(asset("game/c/" + n), StandardCharsets.UTF_8));
                }
                extract("tcc_include", new File(files, "tcc_include"));
                deleteTree(new File(files, "src"));
                say(Native.generate(rom, toml, names.toArray(new String[0]), texts.toArray(new String[0]),
                        new File(files, "src").getPath()));
            } catch (IOException e) {
                say("ERROR: " + e + " (¿elegiste la ROM?)");
            }
        }).start();
    }

    private void bench(String name, int mode, int cpu) {
        File out = new File(getFilesDir(), name);
        deleteTree(out);
        out.mkdirs();
        try {
            new File(out, "pending").createNewFile();
        } catch (IOException e) {
            say("ERROR: " + e);
            return;
        }
        startActivity(new Intent(this, BenchActivity.class).putExtra("out", out.getPath()).putExtra("mode", mode)
                .putExtra("cpu", cpu).putExtra("frames", FRAMES));
    }

    private void showResults() {
        StringBuilder sb = new StringBuilder("Resultados (" + FRAMES + " fotogramas):\n");
        for (String name : new String[] {"run_mem", "run_mem_cpu0", "run_so"}) {
            File r = new File(new File(getFilesDir(), name), "result.txt");
            sb.append("\n[").append(name).append("]\n");
            try {
                sb.append(r.exists() ? new String(Files.readAllBytes(r.toPath()), StandardCharsets.UTF_8) : "(sin medir)\n");
            } catch (IOException e) {
                sb.append(e).append('\n');
            }
        }
        log.setText(sb.toString());
    }

    private byte[] asset(String path) throws IOException {
        try (InputStream in = getAssets().open(path)) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            copy(in, out);
            return out.toByteArray();
        }
    }

    /** Copia la carpeta de assets {@code path} a {@code dest}. */
    private void extract(String path, File dest) throws IOException {
        dest.mkdirs();
        String[] list = getAssets().list(path);
        for (String n : list == null ? new String[0] : list) {
            try (OutputStream out = new FileOutputStream(new File(dest, n))) {
                out.write(asset(path + "/" + n));
            }
        }
    }

    private static long copy(InputStream in, OutputStream out) throws IOException {
        byte[] buf = new byte[1 << 16];
        long total = 0;
        for (int n; (n = in.read(buf)) > 0; total += n) out.write(buf, 0, n);
        return total;
    }

    private static void deleteTree(File f) {
        File[] kids = f.listFiles();
        if (kids != null) for (File k : kids) deleteTree(k);
        f.delete();
    }
}
