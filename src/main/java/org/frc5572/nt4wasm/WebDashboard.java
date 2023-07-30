package org.frc5572.nt4wasm;

import java.io.File;
import java.nio.file.Files;

import edu.wpi.first.wpilibj.Filesystem;

import io.javalin.Javalin;
import io.javalin.http.ContentType;
import io.javalin.http.HttpStatus;

/**
 * A Webserver hosting an NT4 Client Dashboard
 */
public class WebDashboard {

    private static Javalin server;

    /**
     * Start hosting server on port 7070.
     */
    public static void start() {
        server = Javalin.create(/* config */)
                .get("nt4.js", ctx -> {
                    ctx.contentType(ContentType.TEXT_JS)
                            .result(WebDashboard.class.getClassLoader().getResourceAsStream("nt4.js"));
                })
                .get("nt4_wasm_bg.wasm", ctx -> {
                    ctx.contentType("application/wasm")
                            .result(WebDashboard.class.getClassLoader().getResourceAsStream("nt4_wasm_bg.wasm"));
                })
                .get("/*", ctx -> {
                    String path = ctx.path();
                    if (path.endsWith("/")) {
                        path += "index.html";
                    }
                    String[] ext_ = path.split("\\.");
                    String ext = "TXT";
                    if (ext_.length > 0) {
                        ext = ext_[ext_.length - 1].toUpperCase();
                    }
                    File f = new File(new File(Filesystem.getDeployDirectory(), "dashboard"), path);
                    try {
                        byte[] data = Files.readAllBytes(f.toPath());
                        ctx.contentType(ContentType.getContentTypeByExtension(ext)).result(data);
                    } catch (Exception e) {
                        ctx.status(HttpStatus.NOT_FOUND);
                    }
                }).start(7070);

    }

    /**
     * Shut down server.
     */
    public static void stop() {
        server.stop();
    }

}