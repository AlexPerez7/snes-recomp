package io.github.alexperez7.snesspike;

import android.app.Activity;
import android.os.Bundle;
import android.widget.TextView;
import java.io.File;

/**
 * Corre una medición en el proceso ":bench". El runtime termina con exit(): se lleva este proceso y Android vuelve a
 * MainActivity, que muestra result.txt. Si Android recrea esta actividad tras esa salida, la marca "pending" ya no
 * está y se cierra sin medir otra vez.
 */
public class BenchActivity extends Activity {
    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        TextView text = new TextView(this);
        text.setPadding(32, 32, 32, 32);
        setContentView(text);
        String out = getIntent().getStringExtra("out");
        File pending = new File(out, "pending");
        if (out == null || !pending.delete()) {
            finish();
            return;
        }
        int mode = getIntent().getIntExtra("mode", 0);
        int cpu = getIntent().getIntExtra("cpu", -1);
        int frames = getIntent().getIntExtra("frames", 3000);
        text.setText("Midiendo (" + (mode == 0 ? "memoria" : "so+dlopen") + ", núcleo " + cpu + ", " + frames + " fotogramas)…\n"
                + "La app vuelve sola a la pantalla principal al terminar.");
        File files = getFilesDir();
        // 64 MiB de pila: el contexto principal del juego corre en este hilo (en el PC, en el hilo principal de 8 MiB).
        new Thread(null, () -> {
            String r = Native.bench(new File(files, "src").getPath(), new File(files, "tcc_include").getPath(),
                    getApplicationInfo().nativeLibraryDir, new File(files, "rom.sfc").getPath(), frames, cpu, mode, out);
            runOnUiThread(() -> text.setText(r));
        }, "bench", 64L << 20).start();
    }
}
