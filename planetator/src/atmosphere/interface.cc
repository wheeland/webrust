#include "atmosphere/model.h"

#include <memory>
#include <algorithm>
#include <cmath>
#include <map>
#include <stdexcept>
#include <sstream>
#include <string>
#include <vector>
#include <cstring>

using namespace atmosphere;

namespace {

// normalize all parameters to unit sphere radius 1.0
constexpr float MULT = 1.0 / 6360000.0f;

constexpr double kPi = 3.1415926;
constexpr double kSunAngularRadius = 0.00935 / 2.0;
constexpr double kSunSolidAngle = kPi * kSunAngularRadius * kSunAngularRadius;
constexpr double kLengthUnitInMeters = 1.0;

std::unique_ptr<Model> model_;
GLuint full_screen_quad_vao_ = 0;
GLuint full_screen_quad_vbo_ = 0;

// derived from input values
double white_point_r_ = 1.0;
double white_point_g_ = 1.0;
double white_point_b_ = 1.0;
bool do_white_balance_ = false;

}  // anonymous namespace

enum Luminance {
    // Render the spectral radiance at kLambdaR, kLambdaG, kLambdaB.
    NONE,
    // Render the sRGB luminance, using an approximate (on the fly) conversion
    // from 3 spectral radiance values only (see section 14.3 in <a href=
    // "https://arxiv.org/pdf/1612.04336.pdf">A Qualitative and Quantitative
    //  Evaluation of 8 Clear Sky Models</a>).
    APPROXIMATE,
    // Render the sRGB luminance, precomputed from 15 spectral radiance values
    // (see section 4.4 in <a href=
    // "http://www.oskee.wz.cz/stranka/uploads/SCCG10ElekKmoch.pdf">Real-time
    //  Spectral Scattering in Large-scale Natural Participating Media</a>).
    PRECOMPUTED
};

Luminance use_luminance_ = Luminance::NONE;

extern "C" int AtmosphereUseConstantSolarSpectrum = 0;
extern "C" int AtmosphereUseOzone = 1;
extern "C" int AtmosphereUseCombinedTextures = 1;
extern "C" int AtmosphereUseHalfPrecision = 1;

extern "C" float AtmosphereExposure = 10.0;
extern "C" float AtmosphereOuterRadius = 1.05f;
extern "C" float AtmosphereRaleighScattering = 1.0f;
extern "C" float AtmosphereRaleighHeight = 1.0f;
extern "C" float AtmosphereMieScattering = 1.0f;
extern "C" float AtmosphereMieHeight = 1.0f;

extern "C" void AtmosphereInitModel();

extern "C" void AtmosphereInit()
{
#ifdef __EMSCRIPTEN__
#else
    glewInit();
#endif

    glGenVertexArrays(1, &full_screen_quad_vao_);
    glBindVertexArray(full_screen_quad_vao_);
    glGenBuffers(1, &full_screen_quad_vbo_);
    glBindBuffer(GL_ARRAY_BUFFER, full_screen_quad_vbo_);
    const GLfloat vertices[] = {
        -1.0, -1.0, 0.0, 1.0,
        +1.0, -1.0, 0.0, 1.0,
        -1.0, +1.0, 0.0, 1.0,
        +1.0, +1.0, 0.0, 1.0,
    };
    glBufferData(GL_ARRAY_BUFFER, sizeof vertices, vertices, GL_STATIC_DRAW);
    constexpr GLuint kAttribIndex = 0;
    constexpr int kCoordsPerVertex = 4;
    glVertexAttribPointer(kAttribIndex, kCoordsPerVertex, GL_FLOAT, false, 0, 0);
    glEnableVertexAttribArray(kAttribIndex);
    glBindVertexArray(0);

    printf("GL_VENDOR: %s\n", glGetString(GL_VENDOR));
    printf("GL_RENDERER: %s\n", glGetString(GL_RENDERER));
    printf("GL_VERSION: %s\n", glGetString(GL_VERSION));
    printf("GL_SHADING_LANGUAGE_VERSION: %s\n", glGetString(GL_SHADING_LANGUAGE_VERSION));

    AtmosphereInitModel();
}

extern "C" void AtmosphereDestroy()
{
    glDeleteBuffers(1, &full_screen_quad_vbo_);
    glDeleteVertexArrays(1, &full_screen_quad_vao_);
}

extern "C" void AtmosphereInitModel() {
    // Values from "Reference Solar Spectral Irradiance: ASTM G-173", ETR column
    // (see http://rredc.nrel.gov/solar/spectra/am1.5/ASTMG173/ASTMG173.html),
    // summed and averaged in each bin (e.g. the value for 360nm is the average
    // of the ASTM G-173 values for all wavelengths between 360 and 370nm).
    // Values in W.m^-2.
    constexpr int kLambdaMin = 360;
    constexpr int kLambdaMax = 830;
    constexpr double kSolarIrradiance[48] = {
        1.11776, 1.14259, 1.01249, 1.14716, 1.72765, 1.73054, 1.6887, 1.61253,
        1.91198, 2.03474, 2.02042, 2.02212, 1.93377, 1.95809, 1.91686, 1.8298,
        1.8685, 1.8931, 1.85149, 1.8504, 1.8341, 1.8345, 1.8147, 1.78158, 1.7533,
        1.6965, 1.68194, 1.64654, 1.6048, 1.52143, 1.55622, 1.5113, 1.474, 1.4482,
        1.41018, 1.36775, 1.34188, 1.31429, 1.28303, 1.26758, 1.2367, 1.2082,
        1.18737, 1.14683, 1.12362, 1.1058, 1.07124, 1.04992
    };
    // Values from http://www.iup.uni-bremen.de/gruppen/molspec/databases/
    // referencespectra/o3spectra2011/index.html for 233K, summed and averaged in
    // each bin (e.g. the value for 360nm is the average of the original values
    // for all wavelengths between 360 and 370nm). Values in m^2.
    constexpr double kOzoneCrossSection[48] = {
        1.18e-27, 2.182e-28, 2.818e-28, 6.636e-28, 1.527e-27, 2.763e-27, 5.52e-27,
        8.451e-27, 1.582e-26, 2.316e-26, 3.669e-26, 4.924e-26, 7.752e-26, 9.016e-26,
        1.48e-25, 1.602e-25, 2.139e-25, 2.755e-25, 3.091e-25, 3.5e-25, 4.266e-25,
        4.672e-25, 4.398e-25, 4.701e-25, 5.019e-25, 4.305e-25, 3.74e-25, 3.215e-25,
        2.662e-25, 2.238e-25, 1.852e-25, 1.473e-25, 1.209e-25, 9.423e-26, 7.455e-26,
        6.566e-26, 5.105e-26, 4.15e-26, 4.228e-26, 3.237e-26, 2.451e-26, 2.801e-26,
        2.534e-26, 1.624e-26, 1.465e-26, 2.078e-26, 1.383e-26, 7.105e-27
    };
    // From https://en.wikipedia.org/wiki/Dobson_unit, in molecules.m^-2.
    constexpr double kDobsonUnit = 2.687e20;
    // Maximum number density of ozone molecules, in m^-3 (computed so at to get
    // 300 Dobson units of ozone - for this we divide 300 DU by the integral of
    // the ozone density profile defined below, which is equal to 15km).
    constexpr double kOzoneFirstLayer = 15000.0 * MULT;
    constexpr double kOzoneSecondLayer = 25000.0 * MULT;
    constexpr double kMaxOzoneNumberDensity = 300.0 * kDobsonUnit / kOzoneFirstLayer;
    // Wavelength independent solar irradiance "spectrum" (not physically
    // realistic, but was used in the original implementation).
    constexpr double kConstantSolarIrradiance = 1.5;
    const double kBottomRadius = 6360000.0 * MULT;
    const double kTopRadius = 6360000.0 * AtmosphereOuterRadius * MULT;
    const double kRayleigh = 1.24062e-6 * AtmosphereRaleighScattering / MULT;
    const double kRayleighScaleHeight = 8000.0 * AtmosphereRaleighHeight * MULT;
    const double kMieScaleHeight = 1200.0 * AtmosphereMieHeight * MULT;
    const double kMieAngstromAlpha = 0.0;
    const double kMieAngstromBeta = 5.328e-3 * AtmosphereMieScattering;
    const double kMieSingleScatteringAlbedo = 0.9;
    const double kMiePhaseFunctionG = 0.8;
    const double kGroundAlbedo = 0.1;
    const double max_sun_zenith_angle =
        (AtmosphereUseHalfPrecision ? 102.0 : 120.0) / 180.0 * kPi;

    DensityProfileLayer rayleigh_layer(0.0, 1.0, -1.0 / kRayleighScaleHeight, 0.0, 0.0);
    DensityProfileLayer mie_layer(0.0, 1.0, -1.0 / kMieScaleHeight, 0.0, 0.0);
    // Density profile increasing linearly from 0 to 1 between 10 and 25km, and
    // decreasing linearly from 1 to 0 between 25 and 40km. This is an approximate
    // profile from http://www.kln.ac.lk/science/Chemistry/Teaching_Resources/
    // Documents/Introduction%20to%20atmospheric%20chemistry.pdf (page 10).
    std::vector<DensityProfileLayer> ozone_density;
    ozone_density.push_back(DensityProfileLayer(kOzoneSecondLayer, 0.0, 0.0, 1.0 / kOzoneFirstLayer, -2.0 / 3.0));
    ozone_density.push_back(DensityProfileLayer(0.0, 0.0, 0.0, -1.0 / kOzoneFirstLayer, 8.0 / 3.0));

    std::vector<double> wavelengths;
    std::vector<double> solar_irradiance;
    std::vector<double> rayleigh_scattering;
    std::vector<double> mie_scattering;
    std::vector<double> mie_extinction;
    std::vector<double> absorption_extinction;
    std::vector<double> ground_albedo;
    for (int l = kLambdaMin; l <= kLambdaMax; l += 10) {
        double lambda = static_cast<double>(l) * 1e-3;  // micro-meters
        double mie = kMieAngstromBeta / kMieScaleHeight * pow(lambda, -kMieAngstromAlpha);
        wavelengths.push_back(l);
        if (AtmosphereUseConstantSolarSpectrum) {
            solar_irradiance.push_back(kConstantSolarIrradiance);
        } else {
            solar_irradiance.push_back(kSolarIrradiance[(l - kLambdaMin) / 10]);
        }
        rayleigh_scattering.push_back(kRayleigh * pow(lambda, -4));
        mie_scattering.push_back(mie * kMieSingleScatteringAlbedo);
        mie_extinction.push_back(mie);
        absorption_extinction.push_back(AtmosphereUseOzone ?
            kMaxOzoneNumberDensity * kOzoneCrossSection[(l - kLambdaMin) / 10] :
            0.0);
        ground_albedo.push_back(kGroundAlbedo);
    }

    // calculate white point here
    if (do_white_balance_) {
        Model::ConvertSpectrumToLinearSrgb(wavelengths, solar_irradiance,
            &white_point_r_, &white_point_g_, &white_point_b_);
        double white_point = (white_point_r_ + white_point_g_ + white_point_b_) / 3.0;
        white_point_r_ /= white_point;
        white_point_g_ /= white_point;
        white_point_b_ /= white_point;
    }

    model_.reset(
        new Model(
            wavelengths, solar_irradiance, kSunAngularRadius,
            kBottomRadius, kTopRadius, {rayleigh_layer}, rayleigh_scattering,
            {mie_layer}, mie_scattering, mie_extinction, kMiePhaseFunctionG,
            ozone_density, absorption_extinction, ground_albedo, max_sun_zenith_angle,
            kLengthUnitInMeters, use_luminance_ == PRECOMPUTED ? 15 : 3,
            AtmosphereUseCombinedTextures, AtmosphereUseHalfPrecision
        )
    );
    model_->Init();
}

extern "C" int AtmosphereGetShaderSource(char *buffer, int size)
{
    const std::string shader_str =
        std::string(use_luminance_ != NONE ? "#define USE_LUMINANCE\n" : "") +
        "const float kLengthUnitInMeters = " + std::to_string(kLengthUnitInMeters) + ";\n" +
        model_->shaderSource();

    if (size > shader_str.size())
        size = shader_str.size();

    if (buffer != nullptr)
        memcpy(buffer, shader_str.data(), size);

    return shader_str.size();
}

extern "C" void AtmospherePrepareShader(GLuint program, int first_tex_unit)
{
    glUseProgram(program);

    model_->SetProgramUniforms(program, first_tex_unit, first_tex_unit+1, first_tex_unit+2, first_tex_unit+3);
    glUniform3f(glGetUniformLocation(program, "white_point"),
        white_point_r_, white_point_g_, white_point_b_);
    glUniform2f(glGetUniformLocation(program, "sun_size"),
        tan(kSunAngularRadius),
        cos(kSunAngularRadius));

    glUniform1f(glGetUniformLocation(program, "exposure"),
        use_luminance_ != NONE ? AtmosphereExposure * 1e-5 : AtmosphereExposure);
}
