package io.github.alexperez7.snesport;

import android.content.res.AssetManager;
import android.os.Bundle;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import org.libsdl.app.SDLActivity;

/** Ejecuta el juego: SDL arranca libmain.so (launcher.c), que genera y compila el C con la ROM de files/rom.sfc. */
public class GameActivity extends SDLActivity {
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
