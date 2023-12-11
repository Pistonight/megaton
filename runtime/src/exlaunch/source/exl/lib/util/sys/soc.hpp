#pragma once

#include <megaton/assert.hpp>

#include <exl/common.hpp>

namespace exl::util {

    enum class SocType {
        Erista,
        Mariko
    };

    namespace impl {
        inline SocType s_SocType;

        static inline void InitSocType() {
            SplHardwareType hwtype;
            assert_(R_SUCCEEDED(smcGetConfig(SplConfigItem_HardwareType, reinterpret_cast<u64*>(&hwtype))));
                            
            switch (hwtype) {
                case SplHardwareType_Icosa:
                case SplHardwareType_Copper:
                    impl::s_SocType = SocType::Erista;
                    return;
                case SplHardwareType_Hoag:
                case SplHardwareType_Iowa:
                case SplHardwareType_Calcio:
                case SplHardwareType_Aula:
                    impl::s_SocType = SocType::Mariko;
                    return;
                default:
                    unreachable_();
            }
        }
    }

    static inline bool IsSocErista() { return impl::s_SocType == SocType::Erista; }
    static inline bool IsSocMariko() { return impl::s_SocType == SocType::Mariko; }
}
