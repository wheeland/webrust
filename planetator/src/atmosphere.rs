use imgui::*;
use cgmath::{Vector3};
use tinygl::{Program, Texture, Uniform, OffscreenBuffer};

pub struct Atmosphere {
    rel_hr: f32,
    rel_hm: f32,
    beta_r: Vector3<f32>,
    beta_m: Vector3<f32>,
    beta_mul: f32,
    beta_exp: f32,

    sun_optical_depth: Option<Texture>,
    sun_optical_depth_program: Program,
    fsquad: tinygl::shapes::FullscreenQuad,
}

fn glsl_utils() -> String {
    String::from("
    bool solveQuadratic(float a, float b, float c, out float x1, out float x2)
    {
        if (b == 0.0) {
            // Handle special case where the the two vector ray.dir and V are perpendicular
            // with V = ray.orig - sphere.centre
            if (a == 0.0) return false;
            x1 = 0.0;
            x2 = sqrt(-c / a);
            return true;
        }
        float discr = b * b - 4.0 * a * c;

        if (discr < 0.0) return false;

        float q = (b < 0.0) ? -0.5 * (b - sqrt(discr)) : -0.5 * (b + sqrt(discr));
        x1 = q / a;
        x2 = c / q;

        return true;
    }

    bool raySphereIntersect(vec3 orig, vec3 dir, float radius, out float t0, out float t1)
    {
        // They ray dir is normalized so A = 1
        float A = dir.x * dir.x + dir.y * dir.y + dir.z * dir.z;
        float B = 2.0 * (dir.x * orig.x + dir.y * orig.y + dir.z * orig.z);
        float C = orig.x * orig.x + orig.y * orig.y + orig.z * orig.z - radius * radius;

        if (!solveQuadratic(A, B, C, t0, t1)) return false;

        if (t0 > t1) {
            float tt = t0;
            t0 = t1;
            t1 = tt;
        }

        return true;
    }")
}

fn glsl_depthgen_vs() -> String {
    String::from("
    in vec2 vertex;
    out vec2 clipPos;
    void main() {
        clipPos = vertex;
        gl_Position = vec4(vertex, 0.0, 1.0);
    }")
}

fn glsl_depthgen_fs() -> String {
    String::from("
    uniform float relAtmosphereHeight;
    uniform float relHr;
    uniform float relHm;

    in vec2 clipPos;
    out vec2 color;

    ") + &glsl_utils() + "

    vec2 opticalDepth(float relHeight, float angle) {
        vec3 sunDir = vec3(0.0, 0.0, 1.0);
        vec3 pos = (1.0 + relHeight * relAtmosphereHeight) * vec3(sin(angle), 0.0, cos(angle));
        int numSamplesLight = 32;

        float t0Light, t1Light;
        raySphereIntersect(pos, sunDir, 1.0 + relAtmosphereHeight, t0Light, t1Light);
        float segmentLengthLight = t1Light / float(numSamplesLight);
        float tCurrentLight = 0.0;
        float opticalDepthLightR = 0.0, opticalDepthLightM = 0.0;

        for (int j = 0; j < numSamplesLight; ++j) {
            vec3 samplePositionLight = pos + (tCurrentLight + segmentLengthLight * 0.5) * sunDir;
            float heightLight = length(samplePositionLight) - 1.0;
            opticalDepthLightR += exp(-heightLight / relHr) * segmentLengthLight;
            opticalDepthLightM += exp(-heightLight / relHm) * segmentLengthLight;
            tCurrentLight += segmentLengthLight;
        }

        if (relHr <= 0.0) opticalDepthLightR = 0.0;
        if (relHm <= 0.0) opticalDepthLightM = 0.0;

        return vec2(opticalDepthLightR, opticalDepthLightM);
    }

    void main() {
        vec2 tc = vec2(0.5) + 0.5 * clipPos;
        color = opticalDepth(tc.x, 3.14159 * tc.y);
    }"
}

fn glsl_render() -> String {
    String::from("
    uniform float planetRadius;

    uniform float atmosphereRadius;
    uniform float atmosphereHr;
    uniform float atmosphereHm;
    uniform vec3 atmosphereBetaR;
    uniform vec3 atmosphereBetaM;
    uniform sampler2D atmosphereOptDepth;

    ")
    + &glsl_utils() +
    "

    vec3 computeIncidentLight(vec3 orig, vec3 dir, float tmin, float tmax, vec3 sunDirection, vec3 color)
    {
        int numSamples = 16;

        // mie and rayleigh contribution
        vec3 sumR = vec3(0.0);
        vec3 sumM = vec3(0.0);

        float opticalDepthR = 0.0, opticalDepthM = 0.0;
        float mu = dot(dir, sunDirection); // mu in the paper which is the cosine of the angle between the sun direction and the ray direction
        float phaseR = 3.0 / (16.0 * 3.14159) * (1.0 + mu * mu);
        float g = 0.76;
        float phaseM = 3.0 / (8.0 * 3.14159) * ((1.0 - g * g) * (1.0 + mu * mu)) / ((2.0 + g * g) * pow(1.0 + g * g - 2.0 * g * mu, 1.5));

        for (int i = 0; i < numSamples; ++i) {
            float t = mix(tmin, tmax, (float(i) + 0.5) / float(numSamples));
            float segmentLength = (tmax - tmin) / float(numSamples);

            vec3 samplePosition = orig + t * dir;
            float height = max(length(samplePosition) - planetRadius, 0.0);

            // compute optical depth for light
            float hr = (atmosphereHr > 0.0) ? exp(-height / atmosphereHr) * segmentLength : 0.0;
            float hm = (atmosphereHm > 0.0) ? exp(-height / atmosphereHm) * segmentLength : 0.0;
            opticalDepthR += hr;
            opticalDepthM += hm;

            // light optical depth
            float angleToSun = acos(dot(normalize(samplePosition), sunDirection));
            float relAtmHeight = height / (atmosphereRadius - planetRadius);
            vec2 opticalDepthLight = texture(atmosphereOptDepth, vec2(relAtmHeight, angleToSun / 3.14159)).xy;

            vec3 tau = atmosphereBetaR * (opticalDepthR + opticalDepthLight.x) + atmosphereBetaM * 1.1 * (opticalDepthM + opticalDepthLight.y);
            vec3 attenuation = vec3(exp(-tau.x), exp(-tau.y), exp(-tau.z));
            sumR += attenuation * hr;
            sumM += attenuation * hm;
        }

        vec3 ret = (sumR * atmosphereBetaR * phaseR + sumM * atmosphereBetaM * phaseM) * 20.0;

        vec3 tau = atmosphereBetaR * (opticalDepthR) + atmosphereBetaM * 1.1 * (opticalDepthM);
        vec3 attenuation = vec3(exp(-tau.x), exp(-tau.y), exp(-tau.z));
        // TODO: it seems to me that the betaR/M should factor in the transmittance of the far-end color.
        ret += attenuation * color;

        return ret;
    }"
}

impl Atmosphere {
    fn rel_atmosphere_height(&self) -> f32 {
        4.0 * self.rel_hm.max(self.rel_hr)
    }

    fn generate_optical_depth_texture(&self, size: (i32, i32)) -> Texture {
        // Create and Bind target FBO
        let mut buffer = OffscreenBuffer::new(size);
        buffer.add("color", gl::RG32F, gl::RG, gl::FLOAT);
        buffer.bind();

        // Set Parameters
        self.sun_optical_depth_program.bind();
        self.sun_optical_depth_program.uniform("relAtmosphereHeight", Uniform::Float(self.rel_atmosphere_height()));
        self.sun_optical_depth_program.uniform("relHr", Uniform::Float(self.rel_hr));
        self.sun_optical_depth_program.uniform("relHm", Uniform::Float(self.rel_hm));

        // Render
        unsafe { gl::Disable(gl::BLEND); }
        self.fsquad.render(&self.sun_optical_depth_program, "vertex");
        OffscreenBuffer::unbind();

        // Adjust tex params
        let mut tex = buffer.take("color").unwrap();
        tex.filter(gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
        tex.filter(gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);
        tex.wrap(gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE);
        tex.wrap(gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE);
        tex
    }

    pub fn new() -> Self {
        Atmosphere {
            rel_hr: 0.01,
            rel_hm: 0.001,
            beta_r: Vector3::new(0.15, 0.28, 0.5),
            beta_m: Vector3::new(0.4, 0.4, 0.4),
            beta_mul: 40.0,
            beta_exp: 2.0,
            sun_optical_depth: None,
            sun_optical_depth_program: Program::new(&glsl_depthgen_vs(), &glsl_depthgen_fs()),
            fsquad: tinygl::shapes::FullscreenQuad::new(),
        }
    }

    fn beta(&self, beta: Vector3<f32>) -> Vector3<f32> {
        Vector3::new(
            self.beta_mul * beta.x.powf(self.beta_exp),
            self.beta_mul * beta.y.powf(self.beta_exp),
            self.beta_mul * beta.z.powf(self.beta_exp),
        )
    }

    pub fn prepare_shader(&mut self, program: &Program, radius: f32, tex_unit: u32) {
        // (Create and) bind optical depth texture
        // if self.sun_optical_depth.is_none() {
            self.sun_optical_depth = Some(self.generate_optical_depth_texture((128, 128)));
        // }
        self.sun_optical_depth.as_mut().unwrap().bind_at(tex_unit);

        // Set Program uniforms
        program.bind();
        program.uniform("atmosphereOptDepth", Uniform::Signed(tex_unit as i32));
        program.uniform("planetRadius", Uniform::Float(radius));
        program.uniform("atmosphereRadius", Uniform::Float(radius * (1.0 + self.rel_atmosphere_height())));
        program.uniform("atmosphereHr", Uniform::Float(radius * self.rel_hr));
        program.uniform("atmosphereHm", Uniform::Float(radius * self.rel_hm));
        program.uniform("atmosphereBetaR", Uniform::Vec3(self.beta(self.beta_r / radius)));
        program.uniform("atmosphereBetaM", Uniform::Vec3(self.beta(self.beta_m / radius)));
    }

    pub fn shader_source() -> String {
        glsl_render()
    }

    pub fn hr(&self) -> f32 { self.rel_hr }
    pub fn hm(&self) -> f32 { self.rel_hm }
    pub fn beta_r(&self) -> Vector3<f32> { self.beta_r }
    pub fn beta_m(&self) -> Vector3<f32>  { self.beta_m }

    pub fn set_beta_r<V: Into<Vector3<f32>>>(&mut self, v: V) { self.beta_r = v.into(); }
    pub fn set_beta_m<V: Into<Vector3<f32>>>(&mut self, v: V) { self.beta_m = v.into(); }

    pub fn set_hr(&mut self, hr: f32) {
        if self.rel_hr != hr {
            self.rel_hr = hr;
            self.sun_optical_depth = None;
        }
    }

    pub fn set_hm(&mut self, hm: f32) {
        if self.rel_hm != hm {
            self.rel_hm = hm;
            self.sun_optical_depth = None;
        }
    }
}
