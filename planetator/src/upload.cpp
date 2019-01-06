#include <cstdlib>
#include <cstring>
#include <vector>
#include <string>

struct Upload
{
    std::string inputItem;
    std::string fileName;
    std::vector<unsigned char> data;
};

static std::vector<Upload*> s_uploaded;
static std::vector<std::string> s_installedHandlers;

extern "C" char *UploadStart(const char *inputItem)
{
    static char commandBuffer[1024];

    std::string input(inputItem);

    // make sure the handler is installed
    for (auto &h : s_installedHandlers) {
        if (h == input) {
            commandBuffer[0] = 0;
            return commandBuffer;
        }
    }

    s_installedHandlers.push_back(input);

    // execute JS functor
    sprintf(commandBuffer, "\
        var elem = document.getElementById('%s'); \
        elem.addEventListener('input', function() { \
            console.log('Input element changed: %s'); \
            var reader = new FileReader(); \
            reader.addEventListener('loadend', function() { \
                console.log('Upload finished for: %s'); \
                var view = new Uint8Array(reader.result); \
                Module.ccall('UploadFinished', 'void', ['string', 'string', 'array','number'], ['%s', elem.files[0].name, view, view.length]); \
            }); \
            reader.readAsArrayBuffer(elem.files[0]); \
        }); \
        console.log('Installed Event Handler for input element: %s'); \
    ", inputItem, inputItem, inputItem, inputItem, inputItem);

    return commandBuffer;
}

extern "C" void UploadFinished(const char *input, const char *filename, const unsigned char *data, int length)
{
    Upload *upload = new Upload();
    upload->inputItem = std::string(input);
    upload->fileName = std::string(filename);
    upload->data.resize(length);
    memcpy(upload->data.data(), data, length);
    s_uploaded.push_back(upload);
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

