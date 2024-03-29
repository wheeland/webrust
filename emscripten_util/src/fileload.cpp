#include <cstdlib>
#include <cstring>
#include <vector>
#include <string>

#include <emscripten.h>
#include <emscripten/bind.h>
#include <emscripten/val.h>

struct Upload
{
    std::string inputItem;
    std::string fileName;
    std::vector<unsigned char> data;
    bool done = false;
};

static std::vector<Upload*> s_uploaded;
static std::vector<std::string> s_installedHandlers;

extern "C" void UploadStart(const char *inputItem)
{
    static char commandBuffer[1024];

    std::string input(inputItem);

    // make sure the handler is installed
    for (auto &h : s_installedHandlers) {
        if (h == input) {
            return;
        }
    }

    s_installedHandlers.push_back(input);

     sprintf(commandBuffer,
        "try { "
            "var inputItem = '%s'; "
            "var elem = document.getElementById(inputItem); "
            "elem.addEventListener('input', function() { "
            "    var reader = new FileReader(); "
            "    reader.addEventListener('loadend', function() { "
            "        var view = new Uint8Array(reader.result); "
            // "        Module.ccall('UploadFinished', 'void', ['string', 'string', 'array','number'], ['%s', elem.files[0].name, view, view.length]); "
                    "var step = 1024 * 512;"
                    "for (var start = 0; start < view.length; start += step) { "
                        "var end = ((start + step) < view.length) ? (start + step) : view.length; "
                        "var sub = view.slice(start, end);"
                        "Module.ccall('UploadData', 'void', "
                                "['string', 'array', 'number', 'number'], "
                                "[inputItem, sub, start, end]);"
                    "}"
                    "Module.ccall('UploadFinished', 'void', "
                            "['string', 'string'], "
                            "[inputItem, elem.files[0].name]);"
            "    }); "
            "    reader.readAsArrayBuffer(elem.files[0]); "
            "}); "
        "} catch (error) { console.log('Error running JS: ' + error); } "
    , inputItem, inputItem, inputItem, inputItem);

    emscripten_run_script(commandBuffer);
}

extern "C" void UploadData(const char *input, const unsigned char *data, int start, int end)
{
    // get or create Upload object for given input
    Upload *upload;
    if (s_uploaded.size() > 0 && s_uploaded.back()->inputItem == input && !s_uploaded.back()->done) {
        upload = s_uploaded.back();
    } else {
        upload = new Upload();
        upload->inputItem = input;
        s_uploaded.push_back(upload);
    }

    if (upload->data.size() < end)
        upload->data.resize(end);
    memcpy(upload->data.data() + start, data, end-start);
}

extern "C" void UploadFinished(const char *input, const char *filename)
{
    Upload *upload = s_uploaded.back();
    upload->inputItem = std::string(input);
    upload->fileName = std::string(filename);
    upload->done = true;
}

extern "C" int UploadResultSize(const char *inputItem)
{
    // check if there is such a download already
    for (auto up : s_uploaded) {
        if (up->inputItem == inputItem) {
            return up->data.size();
        }
    }
    return -1;
}

extern "C" int UploadFilenameSize(const char *inputItem)
{
    // check if there is such a download already
    for (auto up : s_uploaded) {
        if (up->inputItem == inputItem) {
            return up->fileName.size() + 1;
        }
    }
    return -1;
}

extern "C" int UploadGetData(const char *inputItem, char *buffer, int len)
{
    // check if there is such a download already
    for (auto it = s_uploaded.begin(); it != s_uploaded.end(); ++it) {
        Upload *upload = *it;

        if (upload->inputItem == inputItem) {
            if (len != upload->data.size())
                return 0;

            memcpy(buffer, upload->data.data(), len);
            delete upload;
            s_uploaded.erase(it);
            return 1;
        }
    }

    return 0;
}

extern "C" int UploadGetFilename(const char *inputItem, char *buffer, int len)
{
    // check if there is such a download already
    for (auto it = s_uploaded.begin(); it != s_uploaded.end(); ++it) {
        Upload *upload = *it;

        if (upload->inputItem == inputItem) {
            if (len != upload->fileName.size() + 1)
                return 0;

            memcpy(buffer, upload->fileName.c_str(), len);
            return 1;
        }
    }

    return 0;
}

static std::vector<unsigned char> s_downloadData;

extern "C" void DoDownload(const char *name, int namelen, const char *data, int size)
{
    s_downloadData.resize(size);
    memcpy(s_downloadData.data(), data, size);

    std::string nameStr(name, namelen);

    char commandBuffer[16 * 1024];
    sprintf(commandBuffer,
        "try { eval(\" "
            "var data = Module.DownloadGetData(); "
            "var dataBlob = new Blob([data], {type: 'application/octet-stream'}); "
            "var dataUrl = URL.createObjectURL(dataBlob); "

            // create a/href and link data
            "var element = document.createElement('a');"
            "element.setAttribute('href', dataUrl);"
            "element.setAttribute('download', '%s');"
            "element.style.display = 'none';"
            "document.body.appendChild(element);"
            "element.click();"
            "document.body.removeChild(element);"
        "\") } catch (error) { console.log('Error running JS: ' + error); } "
    , nameStr.c_str());

    emscripten_run_script(commandBuffer);
}

emscripten::val DownloadGetData() {
    return emscripten::val(emscripten::typed_memory_view(s_downloadData.size(), s_downloadData.data()));
}

EMSCRIPTEN_BINDINGS(download_get_bytes) {
    function("DownloadGetData", &DownloadGetData);
}