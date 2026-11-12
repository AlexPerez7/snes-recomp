package io.github.alekaydevs.snesport;

import android.content.res.AssetManager;
import android.graphics.Color;
import android.os.Bundle;
import android.view.Gravity;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.ProgressBar;
import android.widget.TextView;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import org.libsdl.app.SDLActivity;

/** Ejecuta el juego: SDL arranca libmain.so (launcher.c), que genera y compila el C con la ROM de files/rom.sfc. */
public class GameActivity extends SDLActivity {
    private LinearLayout progress;
    private TextView progressText;

    @Override
    protected String[] getLibraries() {
        return new String[] {"SDL2", "main"};
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // Antes de que SDL lance el hilo nativo: los assets (perfil, C propio y cabeceras de TinyCC) pasan a ficheros.
        try {
            File dest = new File(getFilesDir(), "assets");
            deleteTree(dest);
            extract(getAssets(), "", dest);
        } catch (IOException e) {
            throw new RuntimeException("no puedo copiar los ficheros del juego", e);
        }
        super.onCreate(savedInstanceState);
        // Mientras el hilo nativo genera y compila el juego (de medio segundo a unos segundos) la pantalla estaria en negro.
        progress = new LinearLayout(this);
        progress.setOrientation(LinearLayout.VERTICAL);
        progress.setGravity(Gravity.CENTER);
        progress.setBackgroundColor(Color.BLACK);
        progressText = new TextView(this);
        progressText.setTextColor(Color.WHITE);
        progressText.setTextSize(16);
        progressText.setGravity(Gravity.CENTER);
        progressText.setPadding(48, 32, 48, 0);
        progress.addView(new ProgressBar(this));
        progress.addView(progressText);
        addContentView(progress, new ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        setProgress("@preparing");
    }

    /** Lo llama el codigo nativo (launcher.c) desde su hilo: un texto cambia el aviso, null lo quita. */
    public void setProgress(final String text) {
        runOnUiThread(() -> {
            if (progress == null) return;
            if (text == null) progress.setVisibility(android.view.View.GONE);
            else {
                // El código nativo manda claves («@gen», «@compile») para que el texto salga en el idioma del teléfono.
                int id = text.equals("@preparing") ? R.string.preparing : text.equals("@gen") ? R.string.gen_step
                    : text.equals("@compile") ? R.string.compile_step : 0;
                if (id != 0) progressText.setText(id); else progressText.setText(text);
                progress.setVisibility(android.view.View.VISIBLE);
            }
        });
    }

    /** Lo llama el menu del juego (play.c, «Ajustes de la app»): el lanzador (idioma, actualizaciones, licencias, ROM) encima del juego. */
    public void openSettings() {
        runOnUiThread(() -> startActivity(new android.content.Intent(this, LauncherActivity.class)));
    }

    @Override
    protected void attachBaseContext(android.content.Context base) {
        super.attachBaseContext(Lang.wrap(base));
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        // SDL no admite un segundo SDL_main en el mismo proceso: al cerrar el juego de verdad, el proceso muere
        // y el siguiente arranque parte de cero (con la actividad solo en segundo plano no pasa por aqui).
        if (isFinishing()) android.os.Process.killProcess(android.os.Process.myPid());
    }

    private static void extract(AssetManager am, String path, File dest) throws IOException {
        String[] list = am.list(path);
        if (list != null && list.length > 0) {
            dest.mkdirs();
            for (String n : list) extract(am, path.isEmpty() ? n : path + "/" + n, new File(dest, n));
            return;
        }
        dest.getParentFile().mkdirs();
        try (InputStream in = am.open(path); OutputStream out = new FileOutputStream(dest)) {
            byte[] buf = new byte[1 << 16];
            for (int n; (n = in.read(buf)) > 0; ) out.write(buf, 0, n);
        }
    }

    private static void deleteTree(File f) {
        File[] kids = f.listFiles();
        if (kids != null) for (File k : kids) deleteTree(k);
        f.delete();
    }
}
