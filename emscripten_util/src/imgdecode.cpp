#include <cstdlib>
#include <cstring>
#include <vector>
#include <string>

#include <emscripten.h>
#include <emscripten/bind.h>
#include <emscripten/val.h>

struct Decode
{
    int id;
    std::vector<unsigned char> data;
    std::vector<unsigned char> imageData;
    int imageWidth = -1;
    int imageHeight = -1;
    bool done = false;
};

static int s_id = 0;
static std::vector<Decode*> s_decodes;

extern "C" int DecodeStart(const char *data, int size)
{
    Decode *decode = new Decode();

    decode->id = s_id++;
    decode->data.resize(size);
    memcpy(decode->data.data(), data, size);

    s_decodes.push_back(decode);

    // execute JS functor
    char commandBuffer[16 * 1024];
    sprintf(commandBuffer,
        "try { eval(\" "
            "var size = %d; "
            "var data = Module.DecodeGetData(); "
            "var dataBlob = new Blob([data], {type: 'application/octet-stream'}); "
            "var dataUrl = URL.createObjectURL(dataBlob); "

            // // create image and load it
            "var img = document.createElement('img'); "
            "img.onload = function() { "
                // create canvas and paint image
                "var canvas = document.createElement('canvas'); "
                "canvas.width = img.width; "
                "canvas.height = img.height; "
                // "console.log('created canvas'); "

                "var ctx = canvas.getContext('2d'); "
                "ctx.drawImage(img, 0, 0); "
                // "console.log('drawn image'); "

                "var imageData = ctx.getImageData(0, 0, img.width, img.height); "
                // "console.log('gotten imgdata'); "

                "var step = 1024 * 512;"
                "for (var start = 0; start < imageData.data.length; start += step) { "
                    "var end = ((start + step) < imageData.data.length) ? (start + step) : imageData.data.length; "
                    "var sub = imageData.data.slice(start, end);"
                    "Module.ccall('DecodeSetImageData', 'void', "
                            "['number', 'array', 'number', 'number'], "
                            "[%d, sub, start, end]);"
                "}"
                "console.log('Decoded image with size ' + img.width + 'x' + img.height); "
                "Module.ccall('DecodeSetImageDone', 'void', "
                        "['number', 'number', 'number'], "
                        "[%d, img.width, img.height]);"
            "}; "
            "img.onerror = function() { "
                "console.log('Failed to decode image'); "
                "Module.ccall('DecodeSetImageDone', 'void', "
                        "['number', 'number', 'number'], "
                        "[%d, -1, -1]);"
            "};"
            "img.src = dataUrl; "
        "\"); } catch (error) { console.log('Error running JS: ' + error); } "
    , size, decode->id, decode->id, decode->id);

    emscripten_run_script(commandBuffer);

    return decode->id;
}

emscripten::val DecodeGetData() {
    Decode *decode = s_decodes.back();
    return emscripten::val(emscripten::typed_memory_view(decode->data.size(), decode->data.data()));
}

EMSCRIPTEN_BINDINGS(decode_get_bytes_getter) {
    function("DecodeGetData", &DecodeGetData);
}

extern "C" void DecodeSetImageData(int id, char *buffer, int start, int end)
{
    for (Decode *decode : s_decodes) {
        if (decode->id == id) {
            if (decode->imageData.size() < end)
                decode->imageData.resize(end);
            memcpy(decode->imageData.data() + start, buffer, end-start);
            return;
        }
    }
    printf("DecodeSetImageData: no such ID found: %d\n", id);
}

extern "C" void DecodeSetImageDone(int id, int w, int h)
{
    for (Decode *decode : s_decodes) {
        if (decode->id == id) {
            decode->imageWidth = w;
            decode->imageHeight = h;
            decode->done = true;
            return;
        }
    }
    printf("DecodeSetImageData: no such ID found: %d\n", id);
}

extern "C" int DecodeGetResultSize(int id)
{
    for (Decode *decode : s_decodes) {
        if (decode->id == id && decode->done) {
            return decode->imageData.size();
        }
    }
    return -1;
}

extern "C" int DecodeGetResult(int id, unsigned char *buffer, int size, int *width, int *height)
{
    for (auto it = s_decodes.begin(); it != s_decodes.end(); ++it) {
        Decode *decode = *it;

        if (decode->id == id && decode->done) {
            if (size > decode->imageData.size())
                size = decode->imageData.size();

            memcpy(buffer, decode->imageData.data(), size);
            *width = decode->imageWidth;
            *height = decode->imageHeight;

            // remove from vector
            s_decodes.erase(it);
            delete decode;

            return size;
        }
    }
    return -1;
}